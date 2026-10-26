use tree_sitter::Node;

use super::{CALL_ARGUMENTS, DICTIONARY};
use crate::fix::Fix;
use crate::lint::python::ast::{ARGUMENTS_FIELD, FUNCTION_FIELD, OBJECT_FIELD, named, unparenthesized};
use crate::lint::python::escapes::text_value;
use crate::lint::python::semantic::contains_effect;

const COMMA: &str = ",";
const BRACES: [char; 2] = ['{', '}'];

// F504: las claves que el formato no usa, inseguro si el diccionario tiene algo con efectos.
pub fn unused_keys_fix(dictionary: Node<'_>, removed: &[Node<'_>], source: &[u8]) -> Fix {
    let fix = without_elements(dictionary, dictionary, removed, source);
    unsafe_if(fix, has_effect(dictionary, source))
}

// F522 y F523. Sin argumentos que queden y sin llaves en el string, ruff deja sólo el string: así
// `"{{}}".format(1)` conserva la llamada, que cambia `{{` por `{`.
pub fn unused_arguments_fix(call: Node<'_>, removed: &[Node<'_>], source: &[u8]) -> Option<Fix> {
    let arguments = call
        .child_by_field_name(ARGUMENTS_FIELD)
        .filter(|arguments| arguments.kind() == CALL_ARGUMENTS)?;
    let format_string = call
        .child_by_field_name(FUNCTION_FIELD)?
        .child_by_field_name(OBJECT_FIELD)?;
    let effects = removed.iter().any(|argument| has_effect(*argument, source));
    let removes_all = named(arguments).len() == removed.len();
    let string = unparenthesized(format_string);
    let fix = if removes_all && text_value(string, source).is_some_and(|value| !value.contains(BRACES)) {
        let text = String::from_utf8_lossy(source.get(string.byte_range()).unwrap_or_default()).into_owned();
        Fix::replace(call, &text, source)
    } else {
        without_elements(call, arguments, removed, source)
    };
    Some(unsafe_if(fix, effects))
}

// El `retain` de libcst sobre los elementos: cada uno se lleva su coma y el espacio hasta el
// siguiente. El último de una llamada también se lleva el espacio hasta el `)`, que en libcst es
// suyo; en un diccionario ese espacio es de la llave que cierra.
fn without_elements(outer: Node<'_>, container: Node<'_>, removed: &[Node<'_>], source: &[u8]) -> Fix {
    let elements = named(container);
    let closing = container
        .child(container.child_count().saturating_sub(1))
        .map_or_else(|| container.end_byte(), |closing| closing.start_byte());
    let is_dictionary = container.kind() == DICTIONARY;
    let mut replacement = Vec::new();
    let mut cursor = outer.start_byte();
    for (position, element) in elements.iter().enumerate() {
        if !removed.contains(element) {
            continue;
        }
        let comma = element.next_sibling().filter(|sibling| sibling.kind() == COMMA);
        let end = match (elements.get(position.saturating_add(1)), comma) {
            (Some(next), _) => next.start_byte(),
            (None, Some(comma)) if is_dictionary => comma.end_byte(),
            (None, None) if is_dictionary => element.end_byte(),
            (None, _) => closing,
        };
        replacement.extend_from_slice(source.get(cursor..element.start_byte()).unwrap_or_default());
        cursor = end;
    }
    replacement.extend_from_slice(source.get(cursor..outer.end_byte()).unwrap_or_default());
    Fix::replace(outer, &String::from_utf8_lossy(&replacement), source)
}

fn has_effect(node: Node<'_>, source: &[u8]) -> bool {
    contains_effect(node, source, &|_| true)
}

const fn unsafe_if(fix: Fix, condition: bool) -> Fix {
    if condition { fix.marked_unsafe() } else { fix }
}
