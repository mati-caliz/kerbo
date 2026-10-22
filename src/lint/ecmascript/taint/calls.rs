use tree_sitter::Node;

use super::expressions::CLOSURES;
use super::statements::Interpreter;
use super::tables;
use crate::lint::java_receivers::Receiver;
use crate::lint::taint::folding::fold_text_method;
use crate::lint::taint::responses::template_sinks;
use crate::lint::taint::{
    Atom, Flow, Literal, SinkKind, SinkReceiver, State, TaintCall, TaintSink, Value, element,
};
use crate::syntax::{named_children, one_based};

const MEMBER: &str = "member_expression";
const IDENTIFIER: &str = "identifier";
const FUNCTION_FIELD: &str = "function";
const CONSTRUCTOR_FIELD: &str = "constructor";
const ARGUMENTS_FIELD: &str = "arguments";
const OBJECT_FIELD: &str = "object";
const PROPERTY_FIELD: &str = "property";
const COMMENT: &str = "comment";
const SHELL_OPTION: &str = "shell";
const QUALIFIER_SEPARATOR: char = '.';
const NAME_SEPARATORS: &[char] = &['_', '$', '.'];
const ABSORBING_METHODS: &[&str] = &["push", "unshift", "set", "append"];
const LIST_METHODS: &[&str] = &["push", "pop", "shift"];
// Los callbacks que reciben lo que trae el receptor: cada elemento, o lo que resuelve la promesa.
const RECEIVER_CALLBACKS: &[&str] = &[
    "then",
    "map",
    "flatMap",
    "forEach",
    "filter",
    "find",
    "findLast",
    "some",
    "every",
    "reduce",
    "reduceRight",
];

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

// El nombre completo de lo que se llama: por los imports del archivo o las funciones que se ven.
fn qualified(interpreter: &Interpreter<'_, '_>, function: Node<'_>) -> Option<String> {
    let written = interpreter.text(function);
    if !written
        .chars()
        .all(|letter| letter.is_alphanumeric() || NAME_SEPARATORS.contains(&letter))
    {
        return None;
    }
    let (first, rest) = written
        .split_once(QUALIFIER_SEPARATOR)
        .map_or((written, None), |(first, rest)| (first, Some(rest)));
    if let Some(local) = interpreter.locals.get(first) {
        return Some(rest.map_or_else(
            || local.clone(),
            |rest| format!("{local}{QUALIFIER_SEPARATOR}{rest}"),
        ));
    }
    interpreter
        .imports
        .qualify(written)
        .or_else(|| Some(written.to_owned()))
}

// `res.type('json')`, `res.set('Content-Type', 'text/plain')` o
// `res.writeHead(200, { 'content-type': 'application/json' })`: un tipo que no es HTML.
fn declared_not_html(arguments: &[Value]) -> bool {
    let declared = match arguments.last() {
        Some(Value::Map(entries)) => entries
            .iter()
            .find(|(name, _)| tables::is_content_type_header(name))
            .and_then(|(_, value)| value.literal()),
        Some(other) => other.literal(),
        None => None,
    };
    declared
        .is_some_and(|literal| matches!(literal, Literal::Text(written) if !tables::is_html_type(written)))
}

// `{ shell: true }` entre los argumentos de `spawn` o `execFile`.
fn with_shell(arguments: &[Value]) -> bool {
    arguments.iter().any(|argument| {
        element(argument, &Value::Constant(Literal::Text(SHELL_OPTION.to_owned()))).literal()
            == Some(&Literal::Boolean(true))
    })
}

// `"abc".toUpperCase()` con todo constante da otra constante.
fn folded_text_method(receiver: Option<&Value>, method: &str, arguments: &[Value]) -> Option<Literal> {
    let Some(Literal::Text(written)) = receiver.and_then(Value::literal) else {
        return None;
    };
    let constants: Option<Vec<Literal>> = arguments
        .iter()
        .map(|argument| argument.literal().cloned())
        .collect();
    fold_text_method(written, method, &constants?)
}

pub(super) fn call(interpreter: &mut Interpreter<'_, '_>, node: Node<'_>, state: &mut State) -> Value {
    let Some(function) = node
        .child_by_field_name(FUNCTION_FIELD)
        .or_else(|| node.child_by_field_name(CONSTRUCTOR_FIELD))
    else {
        return Value::clean();
    };
    let object = (function.kind() == MEMBER)
        .then(|| function.child_by_field_name(OBJECT_FIELD))
        .flatten();
    let method = match function.child_by_field_name(PROPERTY_FIELD) {
        Some(property) if function.kind() == MEMBER => interpreter.text(property),
        _ => interpreter.text(function),
    };
    let receiver = object.map(|inner| interpreter.expression(inner, state));
    let callback_input = receiver
        .as_ref()
        .filter(|_| RECEIVER_CALLBACKS.contains(&method))
        .map_or_else(Value::clean, |value| Value::Data(value.flow()));
    let arguments = call_arguments(interpreter, node, &callback_input, state);
    let list = object
        .filter(|inner| inner.kind() == IDENTIFIER)
        .map(|inner| interpreter.text(inner));
    if let Some(value) = followed_call(list, method, &arguments, state) {
        return value;
    }
    if let Some(folded) = folded_text_method(receiver.as_ref(), method, &arguments) {
        return Value::Constant(folded);
    }
    let receiver_text = object.map(|inner| interpreter.text(inner)).unwrap_or_default();
    if tables::declares_content_type(receiver_text, method) && declared_not_html(&arguments) {
        interpreter.plain_response = true;
    }
    if (tables::is_request_object(receiver_text) && tables::is_request_method(method))
        || tables::is_hono_source(receiver_text, method)
    {
        return Value::Data(Flow::of(Atom::Source));
    }
    let qualified = qualified(interpreter, function);
    let carried: Vec<Flow> = arguments.iter().map(Value::flow).collect();
    let site = Site {
        function,
        method,
        qualified: qualified.as_deref(),
        receiver: receiver_text,
    };
    record_sinks(interpreter, &site, &arguments, &carried);
    if let Some(escaped) = qualified.as_deref().and_then(tables::escapes) {
        return Value::Data(
            carried
                .iter()
                .fold(Flow::default(), Flow::union)
                .without(escaped.bit()),
        );
    }
    if tables::gives_clean_result(method) {
        return Value::clean();
    }
    let receiver_flow = receiver.as_ref().map(Value::flow).unwrap_or_default();
    if ABSORBING_METHODS.contains(&method)
        && let Some(inner) = object.filter(|inner| inner.kind() == IDENTIFIER)
    {
        let absorbed = carried.iter().fold(receiver_flow, Flow::union);
        state.insert(interpreter.text(inner).to_owned(), Value::Data(absorbed));
        return Value::clean();
    }
    interpreter.calls.push(TaintCall {
        name: method.to_owned(),
        receiver: qualified.map_or(Receiver::Unknown, Receiver::Type),
        receiver_flow,
        arguments: carried,
    });
    Value::Data(Flow::of(Atom::Call(interpreter.calls.len().saturating_sub(1))))
}

// Los callbacks que se pasan corren en el lugar y reciben `callback_input`.
fn call_arguments(
    interpreter: &mut Interpreter<'_, '_>,
    node: Node<'_>,
    callback_input: &Value,
    state: &mut State,
) -> Vec<Value> {
    node.child_by_field_name(ARGUMENTS_FIELD)
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .filter(|argument| argument.kind() != COMMENT)
        .map(|argument| {
            if CLOSURES.contains(&argument.kind()) {
                interpreter.closure(argument, state, callback_input)
            } else {
                interpreter.expression(argument, state)
            }
        })
        .collect()
}

// `lista.push(x)`, `lista.pop()` y `lista.shift()` sobre un arreglo que se sigue por posición.
fn followed_call(list: Option<&str>, method: &str, arguments: &[Value], state: &mut State) -> Option<Value> {
    if !LIST_METHODS.contains(&method) {
        return None;
    }
    let Some(Value::List(items)) = state.get_mut(list?) else {
        return None;
    };
    match (method, arguments) {
        ("push", [item]) => {
            items.push(item.clone());
            Some(Value::clean())
        }
        ("pop", []) => Some(items.pop().unwrap_or_else(Value::clean)),
        ("shift", []) => Some(if items.is_empty() {
            Value::clean()
        } else {
            items.remove(0)
        }),
        _ => None,
    }
}

struct Site<'node, 'text> {
    function: Node<'node>,
    method: &'text str,
    qualified: Option<&'text str>,
    receiver: &'text str,
}

fn record_sinks(
    interpreter: &mut Interpreter<'_, '_>,
    site: &Site<'_, '_>,
    arguments: &[Value],
    carried: &[Flow],
) {
    let Site {
        function,
        method,
        qualified,
        receiver,
    } = *site;
    let at = function.child_by_field_name(PROPERTY_FIELD).unwrap_or(function);
    if tables::renders_template(receiver, method)
        && let (Some(Literal::Text(template)), Some(Value::Map(variables))) =
            (arguments.first().and_then(Value::literal), arguments.get(1))
    {
        interpreter.sinks.extend(template_sinks(at, template, variables));
    }
    let function_kind = qualified.and_then(tables::function_sink);
    let shell = qualified.is_some_and(tables::always_shell) || with_shell(arguments);
    if function_kind == Some(SinkKind::Command) && !shell {
        let program = carried.first().cloned().into_iter().collect();
        interpreter
            .sinks
            .push(sink_at(at, method, SinkKind::Command, program));
        return;
    }
    let sink = function_kind
        .or_else(|| tables::sql_method_sink(receiver, method))
        .or_else(|| {
            tables::response_sink(receiver, method).filter(|kind| {
                *kind != SinkKind::Html || !arguments.iter().all(|argument| matches!(argument, Value::Map(_)))
            })
        })
        .or_else(|| tables::html_method_sink(receiver, method));
    if let Some(kind) = sink {
        interpreter
            .sinks
            .push(sink_at(at, method, kind, carried.to_vec()));
    }
}
