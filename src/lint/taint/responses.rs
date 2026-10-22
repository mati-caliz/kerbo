use super::{Flow, Literal, Value};

const CONTENT_TYPE_HEADER: &str = "content-type";
const HTML_WORD: &str = "html";
// Un escape de HTML propio se reconoce por el nombre (`escape_html`, `escapeHtml`, `HTMLEscape`).
const ESCAPE_WORD: &str = "escape";
// La respuesta de un handler de Lambda: `{ body, headers: { 'Content-Type': 'text/html' } }`, con
// las claves en minúscula (Python y JavaScript) o como campos de Go (`Body`, `Headers`).
const RESPONSE_BODY: &str = "body";
const RESPONSE_HEADERS: &str = "headers";

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
