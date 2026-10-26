use tree_sitter::Node;

use super::type_hints::{subscript_parts, unwrapped};
use crate::fix::{Fix, TextEdit, TextPoint};
use crate::lint::python::ast::{RIGHT_FIELD, text};
use crate::lint::python::branches::equivalent;
use crate::lint::python::codegen;
use crate::lint::python::fix_edits::{Parentheses, commented_unsafe, remove_argument};

const ELEMENT_SEPARATOR: &str = ", ";

// PYI064: `x: Final[Literal[1]] = 1` pasa a `x: Final = 1`, y sin valor `x: Final = 1`; si el valor es
// otro, ruff no lo arregla.
pub fn final_literal_fix(
    final_subscript: Node<'_>,
    literal: Node<'_>,
    assignment: Node<'_>,
    source: &[u8],
) -> Option<Fix> {
    let (head, _) = subscript_parts(final_subscript)?;
    let mut edits = vec![TextEdit {
        start: TextPoint::from_point(head.end_position()),
        end: TextPoint::from_point(final_subscript.end_position()),
        replacement: String::new(),
    }];
    match assignment.child_by_field_name(RIGHT_FIELD) {
        Some(value) if equivalent(unwrapped(value), unwrapped(literal), source) => {}
        Some(_) => return None,
        None => edits.push(TextEdit {
            start: TextPoint::from_point(final_subscript.end_position()),
            end: TextPoint::from_point(final_subscript.end_position()),
            replacement: format!(" = {}", text(literal, source)),
        }),
    }
    Some(Fix::from_edits(source, edits))
}

// PYI062: el `Literal` vuelto a escribir con cada valor una vez, abriendo los `Literal` de adentro.
pub fn literal_dedup_fix(literal: Node<'_>, members: &[Node<'_>], source: &[u8]) -> Option<Fix> {
    let (head, _) = subscript_parts(literal)?;
    let mut unique: Vec<Node<'_>> = Vec::new();
    for member in members {
        if !unique.iter().any(|kept| equivalent(*kept, *member, source)) {
            unique.push(*member);
        }
    }
    let listed: Vec<String> = unique
        .iter()
        .map(|member| codegen::unparse(*member, codegen::COMMA, source))
        .collect();
    let replacement = format!(
        "{}[{}]",
        codegen::unparse(head, codegen::MAX, source),
        listed.join(ELEMENT_SEPARATOR)
    );
    Some(commented_unsafe(
        Fix::replace(literal, &replacement, source),
        literal,
        source,
    ))
}

// PYI059: `Generic[...]` pasa al final de las bases; inseguro, como en ruff.
pub fn generic_last_fix(bases: Node<'_>, generic: Node<'_>, last: Node<'_>, source: &[u8]) -> Fix {
    let mut edits = remove_argument(generic, bases, Parentheses::Preserve, source).edits;
    edits.push(TextEdit {
        start: TextPoint::from_point(last.end_position()),
        end: TextPoint::from_point(last.end_position()),
        replacement: format!("{ELEMENT_SEPARATOR}{}", text(generic, source)),
    });
    Fix::from_edits(source, edits).marked_unsafe()
}
