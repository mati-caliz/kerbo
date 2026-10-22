use tree_sitter::Node;

use super::expressions::literal_of;
use super::statements::Interpreter;
use super::tables;
use super::types::LOCAL_PREFIX;
use crate::lint::java_receivers::Receiver;
use crate::lint::taint::{Atom, Flow, Literal, SinkKind, SinkReceiver, State, TaintCall, TaintSink, Value};
use crate::syntax::{named_children, one_based};

const IDENTIFIER: &str = "identifier";
const SELECTOR: &str = "selector_expression";
const QUALIFIED_TYPE: &str = "qualified_type";
const BINARY: &str = "binary_expression";
const COMMENT: &str = "comment";
const FUNCTION_FIELD: &str = "function";
const TYPE_FIELD: &str = "type";
const ARGUMENTS_FIELD: &str = "arguments";
const OPERAND_FIELD: &str = "operand";
const FIELD_FIELD: &str = "field";
const LEFT_FIELD: &str = "left";
const QUALIFIER_SEPARATOR: char = '.';
// Entre el paquete y lo que se llama de él: la ruta de un paquete puede tener puntos.
const MEMBER_SEPARATOR: char = '#';

pub fn sink_at(node: Node<'_>, callee: &str, kind: SinkKind, arguments: Vec<Flow>) -> TaintSink {
    let start = node.start_position();
    TaintSink {
        callee: callee.to_owned(),
        receiver: SinkReceiver::Known(kind),
        arguments,
        line: one_based(start.row),
        column: one_based(start.column),
    }
}

// A qué se llama: el nombre completo para las tablas, el método, el tipo del receptor y a qué
// función del proyecto puede apuntar (`@f`, `@Tipo.metodo` o `ruta/del/paquete#f`).
pub struct Target<'node> {
    pub qualified: String,
    pub method: String,
    pub receiver_type: Option<String>,
    pub receiver: Option<Node<'node>>,
    pub project: Option<String>,
    pub at: Node<'node>,
}

pub fn call_target<'node>(interpreter: &Interpreter<'_, '_>, node: Node<'node>) -> Option<Target<'node>> {
    let function = node
        .child_by_field_name(FUNCTION_FIELD)
        .or_else(|| node.child_by_field_name(TYPE_FIELD))?;
    match function.kind() {
        SELECTOR => selector_target(interpreter, function),
        IDENTIFIER => {
            let name = interpreter.text(function);
            Some(Target {
                qualified: name.to_owned(),
                method: name.to_owned(),
                receiver_type: None,
                receiver: None,
                project: Some(format!("{LOCAL_PREFIX}{name}")),
                at: function,
            })
        }
        QUALIFIED_TYPE => Some(Target {
            qualified: interpreter.imports.qualify(interpreter.text(function)),
            method: String::new(),
            receiver_type: None,
            receiver: None,
            project: None,
            at: function,
        }),
        _ => None,
    }
}

fn selector_target<'node>(interpreter: &Interpreter<'_, '_>, function: Node<'node>) -> Option<Target<'node>> {
    let operand = function.child_by_field_name(OPERAND_FIELD)?;
    let field = function.child_by_field_name(FIELD_FIELD)?;
    let method = interpreter.text(field).to_owned();
    let written = interpreter.text(operand);
    let package = (operand.kind() == IDENTIFIER && interpreter.types.get(written).is_none())
        .then(|| interpreter.imports.package(written))
        .flatten();
    if let Some(path) = package {
        return Some(Target {
            qualified: format!("{path}{QUALIFIER_SEPARATOR}{method}"),
            project: Some(format!("{path}{MEMBER_SEPARATOR}{method}")),
            method,
            receiver_type: None,
            receiver: None,
            at: field,
        });
    }
    let receiver_type = interpreter.type_of(operand);
    let project = receiver_type.as_deref().map(|typ| {
        typ.strip_prefix(LOCAL_PREFIX).map_or_else(
            || {
                typ.rsplit_once(QUALIFIER_SEPARATOR).map_or_else(
                    || typ.to_owned(),
                    |(path, name)| format!("{path}{MEMBER_SEPARATOR}{name}{QUALIFIER_SEPARATOR}{method}"),
                )
            },
            |local| format!("{LOCAL_PREFIX}{local}{QUALIFIER_SEPARATOR}{method}"),
        )
    });
    Some(Target {
        qualified: receiver_type.as_deref().map_or_else(
            || method.clone(),
            |typ| format!("{typ}{QUALIFIER_SEPARATOR}{method}"),
        ),
        method,
        receiver_type,
        receiver: Some(operand),
        project,
        at: field,
    })
}

fn argument_nodes(node: Node<'_>) -> Vec<Node<'_>> {
    node.child_by_field_name(ARGUMENTS_FIELD)
        .map(named_children)
        .or_else(|| {
            node.child_by_field_name(OPERAND_FIELD)
                .map(|operand| vec![operand])
        })
        .unwrap_or_default()
        .into_iter()
        .filter(|argument| {
            argument.kind() != COMMENT && Some(*argument) != node.child_by_field_name(TYPE_FIELD)
        })
        .collect()
}

pub(super) fn call(interpreter: &mut Interpreter<'_, '_>, node: Node<'_>, state: &mut State) -> Value {
    let nodes = argument_nodes(node);
    let arguments: Vec<Value> = nodes
        .iter()
        .map(|argument| interpreter.expression(*argument, state))
        .collect();
    let carried: Vec<Flow> = arguments.iter().map(Value::flow).collect();
    let Some(target) = call_target(interpreter, node) else {
        return Value::Data(carried.iter().fold(Flow::default(), Flow::union));
    };
    let receiver = target
        .receiver
        .map(|operand| interpreter.expression(operand, state));
    if target
        .receiver_type
        .as_deref()
        .is_some_and(tables::is_context_type)
        && tables::is_context_source(&target.method)
    {
        return Value::Data(Flow::of(Atom::Source));
    }
    if declares_plain_type(interpreter, &target, &arguments) {
        interpreter.plain_response = true;
    }
    record_sinks(interpreter, &target, &nodes, &carried);
    let joined = || carried.iter().fold(Flow::default(), Flow::union);
    if let Some(escaped) = tables::escapes(&target.qualified) {
        return Value::Data(joined().without(escaped.bit()));
    }
    if is_rooted_clean(interpreter, &target, &nodes) {
        return Value::Data(joined().without(SinkKind::Path.bit()));
    }
    if tables::gives_clean_result(&target.method) {
        return Value::clean();
    }
    if tables::is_builder_write(&target.method)
        && let Some(builder) = target.receiver.filter(|operand| operand.kind() == IDENTIFIER)
    {
        let previous = receiver.as_ref().map(Value::flow).unwrap_or_default();
        let written = Value::Data(carried.iter().fold(previous, Flow::union));
        state.insert(interpreter.text(builder).to_owned(), written);
        return Value::clean();
    }
    interpreter.calls.push(TaintCall {
        name: target.method,
        receiver: target.project.map_or(Receiver::Unknown, Receiver::Type),
        receiver_flow: receiver.as_ref().map(Value::flow).unwrap_or_default(),
        arguments: carried,
    });
    Value::Data(Flow::of(Atom::Call(interpreter.calls.len().saturating_sub(1))))
}

// `w.Header().Set("Content-Type", "application/json")`: la respuesta no es HTML. Con un tipo que
// no es constante (un proxy que reenvía el de otro servidor), tampoco se sabe que lo sea.
fn declares_plain_type(interpreter: &Interpreter<'_, '_>, target: &Target<'_>, arguments: &[Value]) -> bool {
    let on_header = target
        .receiver
        .is_some_and(|receiver| interpreter.text(receiver).ends_with(tables::HEADER_CALL));
    let declared = match arguments {
        [name, value] => {
            let content_type = matches!(name.literal(), Some(Literal::Text(header)) if tables::is_content_type_header(header));
            let html =
                matches!(value.literal(), Some(Literal::Text(written)) if tables::is_html_type(written));
            content_type && !html
        }
        _ => false,
    };
    on_header && tables::is_header_setter(&target.method) && declared
}

// `path.Clean("/" + ruta)`: con la raíz adelante no queda ningún `..`.
fn is_rooted_clean(interpreter: &Interpreter<'_, '_>, target: &Target<'_>, nodes: &[Node<'_>]) -> bool {
    tables::ROOTED_CLEANS.contains(&target.qualified.as_str())
        && nodes
            .first()
            .filter(|argument| argument.kind() == BINARY)
            .and_then(|argument| argument.child_by_field_name(LEFT_FIELD))
            .and_then(|left| literal_of(left, interpreter.source))
            .is_some_and(
                |literal| matches!(literal, Literal::Text(written) if written.starts_with(tables::ROOT)),
            )
}

fn record_sinks(
    interpreter: &mut Interpreter<'_, '_>,
    target: &Target<'_>,
    nodes: &[Node<'_>],
    carried: &[Flow],
) {
    let from = |position: usize| carried.get(position..).unwrap_or_default().to_vec();
    let found = sql_sink(interpreter, target)
        .map(|position| (SinkKind::Sql, from(position)))
        .or_else(|| {
            command_sink(interpreter, target, nodes).map(|position| (SinkKind::Command, from(position)))
        })
        .or_else(|| {
            path_sink(target).map(|position| {
                (
                    SinkKind::Path,
                    carried.get(position).cloned().into_iter().collect(),
                )
            })
        })
        .or_else(|| html_sink(interpreter, target, nodes).map(|position| (SinkKind::Html, from(position))));
    if let Some((kind, flows)) = found {
        interpreter
            .sinks
            .push(sink_at(target.at, &target.method, kind, flows));
    }
}

fn sql_sink(interpreter: &Interpreter<'_, '_>, target: &Target<'_>) -> Option<usize> {
    let receiver = target.receiver?;
    let name = interpreter.text(receiver);
    let last = name.rsplit(QUALIFIER_SEPARATOR).next().unwrap_or(name);
    tables::sql_query_position(target.receiver_type.as_deref(), last, &target.method)
}

// `exec.Command("sh", "-c", texto)`: sólo con una shell el texto armado se interpreta.
fn command_sink(interpreter: &Interpreter<'_, '_>, target: &Target<'_>, nodes: &[Node<'_>]) -> Option<usize> {
    if !tables::is_command(&target.qualified) {
        return None;
    }
    let offset = tables::command_offset(&target.qualified);
    let constant = |position: usize| match nodes
        .get(position)
        .and_then(|node| literal_of(*node, interpreter.source))
    {
        Some(Literal::Text(written)) => written,
        _ => String::new(),
    };
    let shell =
        tables::is_shell(&constant(offset)) && tables::is_shell_flag(&constant(offset.saturating_add(1)));
    shell.then(|| offset.saturating_add(2))
}

fn path_sink(target: &Target<'_>) -> Option<usize> {
    let on_context = target
        .receiver_type
        .as_deref()
        .is_some_and(tables::is_context_type)
        && tables::is_context_file_method(&target.method);
    tables::path_position(&target.qualified).or_else(|| on_context.then_some(0))
}

// `w.Write(...)`, `fmt.Fprintf(w, ...)` o `template.HTML(...)`.
fn html_sink(interpreter: &Interpreter<'_, '_>, target: &Target<'_>, nodes: &[Node<'_>]) -> Option<usize> {
    let on_writer = target
        .receiver_type
        .as_deref()
        .is_some_and(tables::is_writer_type)
        && tables::is_writer_method(&target.method);
    if on_writer || tables::is_html_conversion(&target.qualified) {
        return Some(0);
    }
    let to_writer = nodes
        .first()
        .and_then(|writer| interpreter.type_of(*writer))
        .is_some_and(|written| tables::is_writer_type(&written));
    (tables::is_printer(&target.qualified) && to_writer).then_some(1)
}
