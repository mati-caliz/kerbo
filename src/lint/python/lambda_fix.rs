use tree_sitter::Node;

use super::ast::{
    BODY_FIELD, COMMENT, LEFT_FIELD, PARENTHESIZED, RIGHT_FIELD, TYPE_FIELD, named, text, unparenthesized,
};
use super::codegen;
use crate::fix::Fix;

const PARAMETERS_FIELD: &str = "parameters";
const DEFAULT_PARAMETER: &str = "default_parameter";
const VALUE_FIELD: &str = "value";
const NAME_FIELD: &str = "name";
const INDENT: &str = "    ";
const LINE_FEED: char = '\n';
const HASH: u8 = b'#';
const CALLABLE: &str = "Callable";

// El arreglo de E731, como ruff: `f = lambda x: x` pasa a `def f(x):` con `return` y el cuerpo tal como
// está escrito (entre paréntesis si la lambda los tenía), con las líneas siguientes en la sangría de
// la sentencia. Sin arreglo si la sentencia comparte la línea con otra, ni con una anotación
// `Callable` (ruff la pasa a la firma y eso todavía no está). Siempre inseguro.
pub fn lambda_to_def_fix(statement: Node<'_>, assignment: Node<'_>, source: &[u8]) -> Option<Fix> {
    let name = assignment.child_by_field_name(LEFT_FIELD)?;
    let value = assignment.child_by_field_name(RIGHT_FIELD)?;
    let annotation = assignment.child_by_field_name(TYPE_FIELD);
    if annotation.is_some_and(|annotation| text(annotation, source).contains(CALLABLE)) {
        return None;
    }
    let line_start = source
        .get(..statement.start_byte())?
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |position| position.saturating_add(1));
    let indentation = source.get(line_start..statement.start_byte())?;
    if !indentation.iter().all(|byte| matches!(byte, b' ' | b'\t')) {
        return None;
    }
    let after = source.get(statement.end_byte()..)?;
    let rest = after
        .iter()
        .position(|byte| *byte == b'\n')
        .map_or(after, |end| after.get(..end).unwrap_or(after));
    let trailing = rest.trim_ascii_start();
    if !trailing.is_empty() && trailing.first() != Some(&HASH) {
        return None;
    }
    let lambda = unparenthesized(value);
    let body = lambda.child_by_field_name(BODY_FIELD)?;
    let body_text = text(body, source);
    let body_text = if value.kind() == PARENTHESIZED {
        format!("({body_text})")
    } else {
        body_text.to_owned()
    };
    let parameters: Vec<String> = lambda
        .child_by_field_name(PARAMETERS_FIELD)
        .map(named)
        .unwrap_or_default()
        .into_iter()
        .filter(|parameter| parameter.kind() != COMMENT)
        .map(|parameter| parameter_text(parameter, source))
        .collect();
    let generated = format!(
        "def {}({}):\n{INDENT}return {body_text}",
        text(name, source),
        parameters.join(", ")
    );
    let indentation = String::from_utf8_lossy(indentation);
    let indented = generated
        .split(LINE_FEED)
        .collect::<Vec<&str>>()
        .join(&format!("{LINE_FEED}{indentation}"));
    Some(Fix::replace(statement, &indented, source).marked_unsafe())
}

fn parameter_text(parameter: Node<'_>, source: &[u8]) -> String {
    if parameter.kind() != DEFAULT_PARAMETER {
        return text(parameter, source).to_owned();
    }
    let name = parameter
        .child_by_field_name(NAME_FIELD)
        .map_or("", |name| text(name, source));
    let value = parameter
        .child_by_field_name(VALUE_FIELD)
        .map_or_else(String::new, |value| {
            codegen::unparse(value, codegen::COMMA, source)
        });
    format!("{name}={value}")
}
