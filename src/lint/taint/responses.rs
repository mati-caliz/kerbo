use super::{Flow, Literal, SinkKind, SinkReceiver, TaintSink, Value};
use crate::syntax::one_based;

const CONTENT_TYPE_HEADER: &str = "content-type";
const HTML_WORD: &str = "html";
// Un escape de HTML propio se reconoce por el nombre (`escape_html`, `escapeHtml`, `HTMLEscape`).
const ESCAPE_WORD: &str = "escape";
// La respuesta de un handler de Lambda: `{ body, headers: { 'Content-Type': 'text/html' } }`, con
// las claves en minúscula (Python y JavaScript) o como campos de Go (`Body`, `Headers`).
const RESPONSE_BODY: &str = "body";
const RESPONSE_HEADERS: &str = "headers";
// Un sumidero de plantilla lleva en el receptor qué plantilla y qué variable:
// `plantilla:pagina.html#nombre`. El índice decide si esa variable sale sin escapar.
const TEMPLATE_PREFIX: &str = "plantilla:";
const VARIABLE_SEPARATOR: char = '#';

// Lo que se le pasa a una plantilla, variable por variable.
pub fn template_sinks(
    at: tree_sitter::Node<'_>,
    template: &str,
    variables: &[(String, Value)],
) -> Vec<TaintSink> {
    let start = at.start_position();
    variables
        .iter()
        .map(|(variable, value)| TaintSink {
            callee: template.to_owned(),
            receiver: SinkReceiver::Type(format!(
                "{TEMPLATE_PREFIX}{template}{VARIABLE_SEPARATOR}{variable}"
            )),
            arguments: vec![value.flow()],
            line: one_based(start.row),
            column: one_based(start.column),
        })
        .collect()
}

pub fn template_of(receiver: &SinkReceiver) -> Option<(&str, &str)> {
    let SinkReceiver::Type(written) = receiver else {
        return None;
    };
    written
        .strip_prefix(TEMPLATE_PREFIX)?
        .rsplit_once(VARIABLE_SEPARATOR)
}

// Lo que el índice reconoce de un sumidero: los del archivo, y los de una plantilla que escribe esa
// variable sin escapar.
pub fn known_or_template(
    sink: &TaintSink,
    raw_in_template: impl Fn(&TaintSink) -> Option<bool>,
) -> Option<SinkKind> {
    match sink.receiver {
        SinkReceiver::Known(kind) => Some(kind),
        SinkReceiver::Type(_) => raw_in_template(sink).filter(|raw| *raw).map(|_| SinkKind::Html),
        SinkReceiver::StaticField { .. } | SinkReceiver::Constructor(_) => None,
    }
}

pub const fn is_content_type_header(name: &str) -> bool {
    name.eq_ignore_ascii_case(CONTENT_TYPE_HEADER)
}

pub fn is_html_type(written: &str) -> bool {
    written.to_lowercase().contains(HTML_WORD)
}

pub fn is_named_html_escape(simple_name: &str) -> bool {
    let lowered = simple_name.to_lowercase();
    lowered.contains(ESCAPE_WORD) && lowered.contains(HTML_WORD)
}

fn entry<'value>(entries: &'value [(String, Value)], key: &str) -> Option<&'value Value> {
    entries
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(key))
        .map(|(_, value)| value)
}

// Lo que lleva el cuerpo de una respuesta de Lambda que se declara HTML.
pub fn lambda_html_body(response: &Value) -> Option<Flow> {
    let Value::Map(entries) = response else {
        return None;
    };
    let Some(Value::Map(headers)) = entry(entries, RESPONSE_HEADERS) else {
        return None;
    };
    let declared_html = headers.iter().any(|(name, value)| {
        is_content_type_header(name)
            && matches!(value.literal(), Some(Literal::Text(written)) if is_html_type(written))
    });
    declared_html.then(|| entry(entries, RESPONSE_BODY).map(Value::flow).unwrap_or_default())
}
