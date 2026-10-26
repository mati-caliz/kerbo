use std::sync::LazyLock;

use regex::Regex;
use tree_sitter::Node;

use super::{CALL_ARGUMENTS, CONCATENATED_STRING_KIND};
use crate::fix::Fix;
use crate::lint::python::ast::{
    ARGUMENTS_FIELD, COMMENT, FUNCTION_FIELD, OBJECT_FIELD, STRING, is_f_string, named, unparenthesized,
};

// La regex de `format_literals` de ruff, con sus rarezas: el carácter de antes es parte de la
// coincidencia, así que en `"{0}{1}"` el segundo campo no se toca.
static FORMAT_SPECIFIER: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"(?P<prefix>^|[^{]|(?:\{{2})+)\{(?P<int>\d+)(?P<fmt>.*?)}").ok());
const REPLACEMENT: &str = "$prefix{$fmt}";

// UP030: los números de los campos se borran del string y, si no estaban en orden, los argumentos
// se reordenan como en el formato; inseguro, como en ruff.
pub fn explicit_positions_fix(call: Node<'_>, indices: &[usize], source: &[u8]) -> Option<Fix> {
    let pattern = FORMAT_SPECIFIER.as_ref()?;
    let format_string = unparenthesized(
        call.child_by_field_name(FUNCTION_FIELD)?
            .child_by_field_name(OBJECT_FIELD)?,
    );
    let parts = if format_string.kind() == CONCATENATED_STRING_KIND {
        named(format_string)
    } else {
        vec![format_string]
    };
    let mut replacements: Vec<(usize, usize, String)> = parts
        .iter()
        .filter(|part| part.kind() == STRING && !is_f_string(**part, source))
        .map(|part| {
            let written = slice(source, part.start_byte(), part.end_byte());
            (
                part.start_byte(),
                part.end_byte(),
                pattern.replace_all(&written, REPLACEMENT).into_owned(),
            )
        })
        .collect();
    let sequential = indices
        .iter()
        .enumerate()
        .all(|(position, index)| position == *index);
    if !sequential {
        let arguments = call
            .child_by_field_name(ARGUMENTS_FIELD)
            .filter(|arguments| arguments.kind() == CALL_ARGUMENTS)?;
        let given: Vec<Node<'_>> = named(arguments)
            .into_iter()
            .filter(|argument| argument.kind() != COMMENT)
            .collect();
        for (position, index) in indices.iter().enumerate() {
            let target = given.get(position)?;
            let moved = given.get(*index)?;
            replacements.push((
                target.start_byte(),
                target.end_byte(),
                slice(source, moved.start_byte(), moved.end_byte()),
            ));
        }
    }
    replacements.sort_by_key(|(start, _, _)| *start);
    let mut rewritten = String::new();
    let mut cursor = call.start_byte();
    for (start, end, replacement) in replacements {
        rewritten.push_str(&slice(source, cursor, start));
        rewritten.push_str(&replacement);
        cursor = end;
    }
    rewritten.push_str(&slice(source, cursor, call.end_byte()));
    let original = slice(source, call.start_byte(), call.end_byte());
    (rewritten != original).then(|| Fix::replace(call, &rewritten, source).marked_unsafe())
}

fn slice(source: &[u8], start: usize, end: usize) -> String {
    String::from_utf8_lossy(source.get(start..end).unwrap_or_default()).into_owned()
}
