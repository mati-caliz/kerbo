use tree_sitter::Node;

use super::importer::{ImportRequest, ImportStyle, get_or_import_member};
use super::qualified::{is_typing, qualified_name};
use super::type_hints::{Hints, unwrapped};
use crate::fix::{Fix, TextEdit, TextPoint};
use crate::lint::python::ast::{VALUE_FIELD, text};
use crate::lint::python::branches::equivalent;
use crate::lint::python::codegen;
use crate::lint::python::fix_edits::commented_unsafe;

const TYPING: &str = "typing";
const UNION: &str = "Union";
const OPTIONAL: &str = "Optional";
const UNION_TYPE: &str = "union_type";
const NONE_LITERAL: &str = "none";
const NONE_WORD: &str = "None";
const WRAPPERS: [&str; 3] = ["type", "parenthesized_expression", "type_parameter"];
const UNION_SEPARATOR: &str = " | ";
const ELEMENT_SEPARATOR: &str = ", ";

// El arreglo de PYI016: la unión vuelta a escribir sin los repetidos, con `|` si los miembros venían
// de un `|` y con `Union[...]` si no; un solo miembro queda solo. Sin arreglo con un `Optional`
// adentro (ruff lo cuenta como `None`) ni si sólo quedan `None`.
pub fn duplicate_union_fix(hints: Hints<'_, '_, '_>, union: Node<'_>, members: &[Node<'_>]) -> Option<Fix> {
    let source = hints.source;
    let has_optional = members.iter().any(|member| {
        qualified_name(hints.model, typing_head(*member), source)
            .is_some_and(|name| is_typing(&name, OPTIONAL))
    });
    if has_optional {
        return None;
    }
    let mut unique: Vec<Node<'_>> = Vec::new();
    for member in members {
        if !unique.iter().any(|kept| equivalent(*kept, *member, source)) {
            unique.push(*member);
        }
    }
    if unique.iter().all(|member| is_none(*member, source)) {
        return None;
    }
    let mut edits = Vec::new();
    let replacement = if let [only] = unique.as_slice() {
        codegen::unparse(*only, 0, source)
    } else if members.iter().any(|member| comes_from_bit_or(hints, *member)) {
        let mut printed = unique
            .iter()
            .map(|member| codegen::unparse(*member, codegen::BIT_OR.saturating_add(1), source));
        let first = unique
            .first()
            .map(|member| codegen::unparse(*member, codegen::BIT_OR, source));
        printed.next();
        first
            .into_iter()
            .chain(printed)
            .collect::<Vec<String>>()
            .join(UNION_SEPARATOR)
    } else {
        let request = ImportRequest {
            module: TYPING,
            member: UNION,
            style: ImportStyle::ImportFrom,
        };
        let (binding, import) = get_or_import_member(hints.model, source, union, request)?;
        edits.push(import);
        let listed: Vec<String> = unique
            .iter()
            .map(|member| codegen::unparse(*member, codegen::COMMA, source))
            .collect();
        format!("{binding}[{}]", listed.join(ELEMENT_SEPARATOR))
    };
    edits.insert(
        0,
        TextEdit {
            start: TextPoint::from_point(union.start_position()),
            end: TextPoint::from_point(union.end_position()),
            replacement,
        },
    );
    Some(commented_unsafe(Fix::from_edits(source, edits), union, source))
}

fn typing_head(member: Node<'_>) -> Node<'_> {
    member.child_by_field_name(VALUE_FIELD).unwrap_or(member)
}

fn is_none(member: Node<'_>, source: &[u8]) -> bool {
    let member = unwrapped(member);
    member.kind() == NONE_LITERAL || text(member, source) == NONE_WORD
}

fn comes_from_bit_or(hints: Hints<'_, '_, '_>, member: Node<'_>) -> bool {
    std::iter::successors(member.parent(), Node::parent)
        .find(|ancestor| !WRAPPERS.contains(&ancestor.kind()))
        .is_some_and(|parent| parent.kind() == UNION_TYPE || hints.is_bit_or(parent))
}
