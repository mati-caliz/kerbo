use tree_sitter::Node;

use super::calls::{Arguments, Site, qualified};
use super::expressions::{response_body, sink_at};
use super::statements::Interpreter;
use super::tables;
use crate::lint::taint::{Flow, Literal, SinkKind, Value};
use crate::syntax::named_children;

const ARGUMENTS_FIELD: &str = "arguments";
const ATTRIBUTE_FIELD: &str = "attribute";
const ATTRIBUTE: &str = "attribute";
const CALL: &str = "call";
const FUNCTION_FIELD: &str = "function";
const IDENTIFIER: &str = "identifier";
const OBJECT_FIELD: &str = "object";
const SHELL_ARGUMENT: &str = "shell";

// Un objeto de `pathlib`: una variable creada con `Path(...)` (o con `/` sobre otra) o la llamada
// misma.
pub(super) fn is_path_object(interpreter: &Interpreter<'_, '_>, object: Node<'_>) -> bool {
    if object.kind() == IDENTIFIER {
        return interpreter
            .instances
            .get(interpreter.text(object))
            .is_some_and(|created| tables::is_path_object(created));
    }
    object.kind() == CALL
        && object
            .child_by_field_name(FUNCTION_FIELD)
            .and_then(|function| qualified(interpreter, function))
            .is_some_and(|created| tables::is_path_object(&created))
}

// `shell=True` entre los argumentos con nombre.
pub(super) fn with_shell(keywords: &[(&str, Value)]) -> bool {
    keywords
        .iter()
        .any(|(name, value)| *name == SHELL_ARGUMENT && value.literal() == Some(&Literal::Boolean(true)))
}

// Una lista de argumentos que corre una shell con `-c`: `["sh", "-c", comando]`.
pub(super) fn runs_shell(positional: &[Value]) -> bool {
    let parts: Vec<&Value> = positional
        .iter()
        .flat_map(|value| match value {
            Value::List(items) => items.iter().collect::<Vec<_>>(),
            Value::Constant(_) | Value::Data(_) | Value::Map(_) => vec![value],
        })
        .collect();
    let texts: Vec<&str> = parts
        .iter()
        .filter_map(|value| match value.literal() {
            Some(Literal::Text(written)) => Some(written.as_str()),
            _ => None,
        })
        .collect();
    texts.first().is_some_and(|program| tables::is_shell(program))
        && texts.iter().any(|written| tables::is_shell_command_flag(written))
}

// `make_response(cuerpo, ...)`: el sumidero de HTML es sólo el cuerpo.
pub(super) fn response(
    interpreter: &mut Interpreter<'_, '_>,
    site: &Site<'_, '_>,
    node: Node<'_>,
    positional: &[Value],
) -> Option<Flow> {
    let responded = site
        .qualified
        .as_deref()
        .filter(|created| tables::is_response(created))?;
    let body = node
        .child_by_field_name(ARGUMENTS_FIELD)
        .and_then(|arguments| named_children(arguments).into_iter().next())
        .zip(positional.first())
        .map(|(written, value)| response_body(written, value))
        .unwrap_or_default();
    interpreter.sinks.push(sink_at(
        site.function,
        responded,
        SinkKind::Html,
        vec![body.clone()],
    ));
    Some(body)
}

// Sin shell, lo peligroso es el programa: el primero de la lista. Un texto suelto es el nombre del
// programa entero (`"grep -R {}".format(x)` no corre nada), así que no cuenta.
fn program_of(command: Option<&Value>) -> Flow {
    match command {
        Some(Value::List(items)) => items.first().map(Value::flow).unwrap_or_default(),
        Some(Value::Constant(_) | Value::Data(_) | Value::Map(_)) | None => Flow::default(),
    }
}

// Anota los sumideros de la llamada y devuelve lo que llevan sus argumentos.
pub(super) fn record_sinks(
    interpreter: &mut Interpreter<'_, '_>,
    site: &Site<'_, '_>,
    arguments: &Arguments<'_>,
) -> Vec<Flow> {
    let mut carried = arguments.flows();
    let at = site
        .function
        .child_by_field_name(ATTRIBUTE_FIELD)
        .unwrap_or(site.function);
    let qualified = site.qualified.as_deref();
    if qualified.is_some_and(tables::runs_program) {
        let shell = with_shell(&arguments.keywords) || runs_shell(&arguments.positional);
        let flows = if shell {
            carried.clone()
        } else {
            vec![program_of(arguments.positional.first())]
        };
        interpreter
            .sinks
            .push(sink_at(at, site.method, SinkKind::Command, flows));
    }
    if site.object.is_some_and(|inner| on_query(interpreter, inner))
        && let Some(kind) = tables::query_method_sink(site.method)
    {
        interpreter
            .sinks
            .push(sink_at(at, site.method, kind, carried.clone()));
    }
    let on_path = site
        .object
        .is_some_and(|inner| is_path_object(interpreter, inner));
    if on_path {
        carried.push(site.receiver.as_ref().map(Value::flow).unwrap_or_default());
    }
    let sink = qualified
        .and_then(tables::function_sink)
        .or_else(|| site.object.and_then(|_| tables::method_sink(site.method)))
        .or_else(|| on_path.then(|| tables::path_method_sink(site.method)).flatten());
    if let Some(kind) = sink {
        interpreter
            .sinks
            .push(sink_at(at, site.method, kind, carried.clone()));
    }
    carried
}

pub(super) fn on_query(interpreter: &Interpreter<'_, '_>, object: Node<'_>) -> bool {
    object.kind() == IDENTIFIER
        && interpreter
            .instances
            .get(interpreter.text(object))
            .is_some_and(|created| created == tables::QUERY_OBJECT)
}

// Si la llamada arma una sentencia de SQLAlchemy: la cadena empieza en `select(...)`, `insert(...)`
// o en una variable que salió de uno de ellos.
pub(super) fn builds_statement(interpreter: &Interpreter<'_, '_>, site: &Site<'_, '_>) -> bool {
    if site
        .qualified
        .as_deref()
        .is_some_and(tables::is_statement_builder)
    {
        return true;
    }
    let mut current = site.object;
    while let Some(inner) = current {
        match inner.kind() {
            CALL => {
                let function = inner.child_by_field_name(FUNCTION_FIELD);
                if function
                    .and_then(|function| qualified(interpreter, function))
                    .is_some_and(|created| tables::is_statement_builder(&created))
                {
                    return true;
                }
                current = function.and_then(|function| function.child_by_field_name(OBJECT_FIELD));
            }
            ATTRIBUTE => current = inner.child_by_field_name(OBJECT_FIELD),
            _ => current = None,
        }
    }
    false
}
