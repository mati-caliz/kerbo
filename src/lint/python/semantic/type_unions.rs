use tree_sitter::Node;

use super::Mode;
use super::call_fixes::builtin_available;
use super::pep::is_typing_only;
use super::qualified::is_builtin_named;
use super::type_hints::{Hints, subscript_parts, unwrapped};
use crate::fix::Fix;
use crate::lint::python::Report;
use crate::lint::python::ast::{COMMENT, PARENTHESIZED, TUPLE, is_f_string, is_string, named, text};
use crate::lint::python::codegen;
use crate::lint::python::fix_edits::commented_unsafe;
use crate::rules::PythonRule;

const TYPE_NODE: &str = "type";
const UNION_TYPE: &str = "union_type";
const TYPE_BUILTIN: &str = "type";
const MINIMUM_TYPE_MEMBERS: usize = 2;
const UNION: &str = "Union";
const UNION_SEPARATOR: &str = " | ";
const ELEMENT_SEPARATOR: &str = ", ";
const WRAPPERS: [&str; 3] = ["type", "parenthesized_expression", "type_parameter"];

// PYI055 fuera de un string: sólo donde Python no evalúa la anotación (con el import del futuro, en
// un stub o adentro de `if TYPE_CHECKING:`).
pub fn redundant_type_union(
    hints: Hints<'_, '_, '_>,
    node: Node<'_>,
    mode: Mode,
    deferred_annotations: bool,
) -> Option<Report> {
    (has_redundant_type_members(hints, node, mode)
        && is_typing_only(node, deferred_annotations, hints.source))
    .then(|| {
        let fix = merged_type_union(hints, node, mode).map(|replacement| {
            commented_unsafe(Fix::replace(node, &replacement, hints.source), node, hints.source)
        });
        Report::at(PythonRule::PYI055, node, "").with_fix(fix)
    })
}

// El arreglo de PYI055: los `type[...]` juntos en uno, con `|` si la unión venía con `|` y con
// `Union[...]` adentro del `type` si no; sin arreglo si `type` está tapado.
pub fn merged_type_union(hints: Hints<'_, '_, '_>, union: Node<'_>, mode: Mode) -> Option<String> {
    if !builtin_available(hints.model, TYPE_BUILTIN) {
        return None;
    }
    let source = hints.source;
    let mut members = Vec::new();
    hints.union_members(union, mode, &mut members);
    let (types, others): (Vec<Node<'_>>, Vec<Node<'_>>) =
        members.iter().partition(|member| is_type_member(hints, **member));
    let inner: Vec<Node<'_>> = types
        .iter()
        .filter_map(|member| subscript_parts(*member).and_then(|(_, slices)| slices.first().copied()))
        .collect();
    let from_bit_or = members.iter().any(|member| comes_from_bit_or(hints, *member));
    if from_bit_or || !hints.is_typing_subscript(union, UNION) {
        let merged = format!("{TYPE_BUILTIN}[{}]", bit_or_join(&inner, source));
        let rest = others
            .iter()
            .map(|other| codegen::unparse(*other, codegen::BIT_OR.saturating_add(1), source));
        return Some(
            std::iter::once(merged)
                .chain(rest)
                .collect::<Vec<String>>()
                .join(UNION_SEPARATOR),
        );
    }
    let (head, _) = subscript_parts(union)?;
    let head = codegen::unparse(head, codegen::MAX, source);
    let listed: Vec<&str> = inner.iter().map(|member| text(*member, source)).collect();
    let merged = format!("{TYPE_BUILTIN}[{head}[{}]]", listed.join(ELEMENT_SEPARATOR));
    if others.is_empty() {
        return Some(merged);
    }
    let rest = others
        .iter()
        .map(|other| codegen::unparse(*other, codegen::COMMA, source));
    let elements: Vec<String> = std::iter::once(merged).chain(rest).collect();
    Some(format!("{head}[{}]", elements.join(ELEMENT_SEPARATOR)))
}

fn bit_or_join(members: &[Node<'_>], source: &[u8]) -> String {
    members
        .iter()
        .enumerate()
        .map(|(index, member)| {
            let level = if index == 0 {
                codegen::BIT_OR
            } else {
                codegen::BIT_OR.saturating_add(1)
            };
            codegen::unparse(*member, level, source)
        })
        .collect::<Vec<String>>()
        .join(UNION_SEPARATOR)
}

fn comes_from_bit_or(hints: Hints<'_, '_, '_>, member: Node<'_>) -> bool {
    std::iter::successors(member.parent(), Node::parent)
        .find(|ancestor| !WRAPPERS.contains(&ancestor.kind()))
        .is_some_and(|parent| parent.kind() == UNION_TYPE || hints.is_bit_or(parent))
}

// PYI055: una unión (la de afuera, no las anidadas) con más de un `type[...]`, que se juntan en uno.
// Quien llama decide si la unión está en un contexto de tipos: ruff no la marca donde se evalúa.
pub fn has_redundant_type_members(hints: Hints<'_, '_, '_>, node: Node<'_>, mode: Mode) -> bool {
    if matches!(node.kind(), TYPE_NODE | PARENTHESIZED)
        || !hints.is_union(node, mode)
        || hints.is_nested(node, mode, Hints::is_union)
    {
        return false;
    }
    let mut members = Vec::new();
    hints.union_members(node, mode, &mut members);
    members
        .iter()
        .filter(|member| is_type_member(hints, **member))
        .count()
        >= MINIMUM_TYPE_MEMBERS
}

// `type[X]` del builtin, con un solo argumento que no es una tupla.
fn is_type_member(hints: Hints<'_, '_, '_>, member: Node<'_>) -> bool {
    subscript_parts(member).is_some_and(|(value, slices)| {
        matches!(slices.as_slice(), [only] if unwrapped(*only).kind() != TUPLE)
            && is_builtin_named(hints.model, value, hints.source, TYPE_BUILTIN)
    })
}

// TC010: un string en una unión `X | Y` que Python evalúa, donde `"A" | None` falla con TypeError.
// Avisa en cada string; no en un stub, adentro de una anotación entre comillas ni en una que no se
// evalúa.
pub fn runtime_string_union(
    hints: Hints<'_, '_, '_>,
    node: Node<'_>,
    mode: Mode,
    deferred: Deferral,
) -> Vec<Report> {
    if mode != Mode::TypeDefinition
        || deferred.always
        || matches!(node.kind(), TYPE_NODE | PARENTHESIZED)
        || !is_bit_or_union(hints, node)
        || hints.is_nested(node, mode, Hints::is_union)
        || is_typing_only(node, deferred.future_annotations, hints.source)
    {
        return Vec::new();
    }
    let mut strings = Vec::new();
    bit_or_strings(hints, node, &mut strings);
    strings
        .into_iter()
        .map(|string| Report::at(PythonRule::TC010, string, ""))
        .collect()
}

#[derive(Debug, Clone, Copy)]
pub struct Deferral {
    pub always: bool,
    pub future_annotations: bool,
}

fn is_bit_or_union(hints: Hints<'_, '_, '_>, node: Node<'_>) -> bool {
    node.kind() == UNION_TYPE || hints.is_bit_or(node)
}

fn bit_or_strings<'tree>(hints: Hints<'_, '_, '_>, node: Node<'tree>, strings: &mut Vec<Node<'tree>>) {
    let node = unwrapped(node);
    if is_bit_or_union(hints, node) {
        for side in named(node).into_iter().filter(|side| side.kind() != COMMENT) {
            bit_or_strings(hints, side, strings);
        }
    } else if is_string(node) && !is_f_string(node, hints.source) {
        strings.push(node);
    }
}
