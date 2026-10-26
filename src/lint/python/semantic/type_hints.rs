use tree_sitter::Node;

use super::Mode;
use super::model::Model;
use super::qualified::{is_typing, qualified_name};
use super::union_duplicates::duplicate_union_fix;
use crate::fix::Fix;
use crate::lint::python::Report;
use crate::lint::python::ast::{
    ASSIGNMENT, ATTRIBUTE, COMMENT, KEYWORD_ARGUMENT, LEFT_FIELD, PARENTHESIZED, RIGHT_FIELD, SUBSCRIPT,
    SUBSCRIPT_FIELD, TUPLE, VALUE_FIELD, fields, named, text, unparenthesized,
};
use crate::lint::python::branches::equivalent;
use crate::lint::python::fix_edits::commented_unsafe;
use crate::rules::PythonRule;

const TYPE_NODE: &str = "type";
const TYPE_PARAMETER: &str = "type_parameter";
const GENERIC_TYPE: &str = "generic_type";
const UNION_TYPE: &str = "union_type";
const BINARY_OPERATOR: &str = "binary_operator";
const ARGUMENT_LIST: &str = "argument_list";
const CLASS: &str = "class_definition";
const SUPERCLASSES_FIELD: &str = "superclasses";
const OPERATOR_FIELD: &str = "operator";
const TYPE_FIELD: &str = "type";
const BIT_OR: &str = "|";
const NONE_LITERAL: &str = "none";
const NONE_WORD: &str = "None";
const UNION: &str = "Union";
const LITERAL: &str = "Literal";
const NEVER_TYPES: [&str; 2] = ["Never", "NoReturn"];
const FINAL: &str = "Final";
const GENERIC: &str = "Generic";
const SPLATS: [&str; 2] = ["list_splat", "dictionary_splat"];
const BYTE_STRINGS: [&str; 3] = [
    "typing.ByteString",
    "typing_extensions.ByteString",
    "collections.abc.ByteString",
];
const WRAPPERS: [&str; 3] = [TYPE_NODE, PARENTHESIZED, TYPE_PARAMETER];
const LITERAL_VALUES: [&str; 7] = [
    "string",
    "concatenated_string",
    "integer",
    "float",
    "true",
    "false",
    NONE_LITERAL,
];

#[derive(Clone, Copy)]
pub struct Hints<'model, 'tree, 'source> {
    pub model: &'model Model<'tree>,
    pub source: &'source [u8],
}

// Las reglas de `flake8-pyi` sobre una expresión: PYI016 y PYI030 en una unión, PYI061 y PYI062 en un
// `Literal`, PYI064 en `Final[Literal[...]]`, PYI057 en `typing.ByteString` y PYI059 en las bases de
// una clase.
pub fn type_hint_reports(model: &Model<'_>, source: &[u8], node: Node<'_>, mode: Mode) -> Vec<Report> {
    if matches!(node.kind(), TYPE_NODE | PARENTHESIZED) {
        return Vec::new();
    }
    let hints = Hints { model, source };
    let mut reports = Vec::new();
    if hints.is_union(node, mode) && !hints.is_nested(node, mode, Hints::is_union) {
        let mut members = Vec::new();
        hints.union_members(node, mode, &mut members);
        let repeated = duplicates(&members, source, PythonRule::PYI016);
        let fix = if repeated.is_empty() {
            None
        } else {
            duplicate_union_fix(hints, node, &members)
        };
        reports.extend(repeated.into_iter().map(|report| report.with_fix(fix.clone())));
        if members
            .iter()
            .filter(|member| hints.is_typing_subscript(**member, LITERAL))
            .count()
            > 1
        {
            reports.push(Report::at(PythonRule::PYI030, node, ""));
        }
    }
    if hints.is_typing_subscript(node, LITERAL)
        && !hints.is_nested(node, mode, |hints, parent, _| {
            hints.is_typing_subscript(parent, LITERAL)
        })
    {
        let mut members = Vec::new();
        hints.literal_members(node, &mut members);
        let nested = subscript_parts(node)
            .map(|(_, slices)| slices)
            .unwrap_or_default()
            .into_iter()
            .any(|slice| hints.is_typing_subscript(slice, LITERAL));
        if nested {
            reports.push(Report::at(PythonRule::RUF041, node, ""));
        }
        reports.extend(
            members
                .iter()
                .filter(|member| member.kind() == NONE_LITERAL)
                .map(|member| Report::at(PythonRule::PYI061, *member, "")),
        );
        reports.extend(duplicates(&members, source, PythonRule::PYI062));
    }
    reports.extend(hints.never_in_union(node));
    reports.extend(hints.redundant_final_literal(node));
    if node.kind() == ATTRIBUTE
        && qualified_name(model, node, source).is_some_and(|name| BYTE_STRINGS.contains(&name.as_str()))
    {
        reports.push(Report::at(PythonRule::PYI057, node, ""));
    }
    reports.extend(hints.generic_not_last(node));
    reports
}

fn duplicates(members: &[Node<'_>], source: &[u8], rule: PythonRule) -> Vec<Report> {
    let mut reports = Vec::new();
    for (index, member) in members.iter().enumerate() {
        let seen = members
            .iter()
            .take(index)
            .any(|earlier| equivalent(*earlier, *member, source));
        if seen {
            reports.push(Report::at(rule, *member, ""));
        }
    }
    reports
}

pub fn unwrapped(node: Node<'_>) -> Node<'_> {
    let mut current = node;
    while matches!(current.kind(), TYPE_NODE | PARENTHESIZED) {
        match named(current).into_iter().find(|child| child.kind() != COMMENT) {
            Some(inner) => current = inner,
            None => break,
        }
    }
    current
}

// El valor y los elementos de `X[a, b]`, tanto en una expresión como en una anotación.
pub fn subscript_parts(node: Node<'_>) -> Option<(Node<'_>, Vec<Node<'_>>)> {
    let node = unwrapped(node);
    match node.kind() {
        SUBSCRIPT => {
            let slices = fields(node, SUBSCRIPT_FIELD);
            let slices = match slices.as_slice() {
                [only] if unparenthesized(*only).kind() == TUPLE => named(unparenthesized(*only)),
                _ => slices,
            };
            Some((node.child_by_field_name(VALUE_FIELD)?, slices))
        }
        GENERIC_TYPE => {
            let parts = named(node);
            let value = *parts.first()?;
            let slices = parts
                .iter()
                .skip(1)
                .flat_map(|parameters| named(*parameters))
                .filter(|slice| slice.kind() != COMMENT)
                .collect();
            Some((value, slices))
        }
        _ => None,
    }
}

impl<'tree> Hints<'_, '_, '_> {
    pub fn is_typing_subscript(self, node: Node<'_>, member: &str) -> bool {
        subscript_parts(node).is_some_and(|(value, _)| {
            qualified_name(self.model, value, self.source).is_some_and(|name| is_typing(&name, member))
        })
    }

    pub fn is_bit_or(self, node: Node<'_>) -> bool {
        node.kind() == BINARY_OPERATOR
            && node
                .child_by_field_name(OPERATOR_FIELD)
                .is_some_and(|operator| text(operator, self.source) == BIT_OR)
    }

    pub fn is_union(self, node: Node<'_>, mode: Mode) -> bool {
        let node = unwrapped(node);
        node.kind() == UNION_TYPE
            || (mode == Mode::TypeDefinition && self.is_bit_or(node))
            || self.is_typing_subscript(node, UNION)
    }

    // `in_nested_union` y compañía: el padre, sin contar paréntesis ni envoltorios, también lo es.
    pub fn is_nested(self, node: Node<'_>, mode: Mode, check: impl Fn(Self, Node<'_>, Mode) -> bool) -> bool {
        let mut parent = node.parent();
        while let Some(current) = parent.filter(|current| {
            WRAPPERS.contains(&current.kind())
                || (current.kind() == TUPLE
                    && current.parent().is_some_and(|outer| outer.kind() == SUBSCRIPT))
        }) {
            parent = current.parent();
        }
        parent.is_some_and(|parent| check(self, parent, mode))
    }

    pub fn union_members(self, node: Node<'tree>, mode: Mode, members: &mut Vec<Node<'tree>>) {
        let node = unwrapped(node);
        if node.kind() == UNION_TYPE {
            for side in named(node) {
                self.union_members(side, mode, members);
            }
        } else if self.is_bit_or(node) && mode == Mode::TypeDefinition {
            let sides = [LEFT_FIELD, RIGHT_FIELD]
                .into_iter()
                .filter_map(|side| node.child_by_field_name(side));
            for side in sides {
                self.union_members(side, mode, members);
            }
        } else if self.is_typing_subscript(node, UNION) {
            for slice in subscript_parts(node)
                .map(|(_, slices)| slices)
                .unwrap_or_default()
            {
                self.union_members(slice, Mode::TypeDefinition, members);
            }
        } else {
            members.push(node);
        }
    }

    fn literal_members(self, node: Node<'tree>, members: &mut Vec<Node<'tree>>) {
        for slice in subscript_parts(node)
            .map(|(_, slices)| slices)
            .unwrap_or_default()
        {
            let slice = unwrapped(slice);
            if self.is_typing_subscript(slice, LITERAL) {
                self.literal_members(slice, members);
            } else {
                members.push(slice);
            }
        }
    }

    // RUF020: `Never | int` o `Union[NoReturn, int]`. Como ruff, mira los dos lados de cada `|` y los
    // elementos de `Union`, en cualquier contexto.
    fn never_in_union(self, node: Node<'_>) -> Vec<Report> {
        let operands = if node.kind() == UNION_TYPE || self.is_bit_or(node) {
            named(node)
                .into_iter()
                .filter(|part| part.kind() != COMMENT)
                .collect()
        } else if self.is_typing_subscript(node, UNION) {
            subscript_parts(node)
                .map(|(_, slices)| slices)
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        operands
            .into_iter()
            .map(unwrapped)
            .filter(|operand| {
                qualified_name(self.model, *operand, self.source)
                    .is_some_and(|name| NEVER_TYPES.iter().any(|never| is_typing(&name, never)))
            })
            .map(|operand| {
                Report::at(PythonRule::RUF020, operand, "").with_fix(self.never_union_fix(node, operand))
            })
            .collect()
    }

    // El arreglo de RUF020 para `Never | T`: queda el otro lado, tal como está escrito. Ruff no lo
    // arregla si quedaría `None | None`, y el de `Union[...]` todavía no está.
    fn never_union_fix(self, union: Node<'_>, never: Node<'_>) -> Option<Fix> {
        if union.kind() != UNION_TYPE && !self.is_bit_or(union) {
            return None;
        }
        let sides: Vec<Node<'_>> = named(union)
            .into_iter()
            .filter(|part| part.kind() != COMMENT)
            .collect();
        let [left, right] = sides.as_slice() else {
            return None;
        };
        let other = if unwrapped(*left).id() == never.id() {
            *right
        } else {
            *left
        };
        if self.has_bare_none(union) {
            return None;
        }
        let fix = Fix::replace(union, text(unwrapped(other), self.source), self.source);
        Some(commented_unsafe(fix, union, self.source))
    }

    fn has_bare_none(self, union: Node<'_>) -> bool {
        let root = std::iter::successors(Some(union), |node| {
            node.parent()
                .filter(|parent| parent.kind() == UNION_TYPE || self.is_bit_or(*parent))
        })
        .last()
        .unwrap_or(union);
        let mut pending = vec![root];
        while let Some(node) = pending.pop() {
            if node.kind() == UNION_TYPE || self.is_bit_or(node) {
                pending.extend(named(node));
            } else if unwrapped(node).kind() == NONE_LITERAL
                || text(unwrapped(node), self.source) == NONE_WORD
            {
                return true;
            }
        }
        false
    }

    // PYI064: `x: Final[Literal[1]] = ...`, donde alcanza con `Final`. Avisa en la asignación.
    fn redundant_final_literal(self, node: Node<'_>) -> Option<Report> {
        let (_, slices) = subscript_parts(node).filter(|_| self.is_typing_subscript(node, FINAL))?;
        let [literal] = slices.as_slice() else {
            return None;
        };
        let (_, values) =
            subscript_parts(*literal).filter(|_| self.is_typing_subscript(*literal, LITERAL))?;
        let [value] = values.as_slice() else {
            return None;
        };
        if !LITERAL_VALUES.contains(&unwrapped(*value).kind()) {
            return None;
        }
        let annotation = std::iter::successors(node.parent(), Node::parent)
            .find(|ancestor| ancestor.kind() != PARENTHESIZED)
            .filter(|ancestor| ancestor.kind() == TYPE_NODE)?;
        let assignment = annotation.parent().filter(|parent| {
            parent.kind() == ASSIGNMENT
                && parent
                    .child_by_field_name(TYPE_FIELD)
                    .is_some_and(|typed| typed.id() == annotation.id())
        })?;
        Some(Report::at(PythonRule::PYI064, assignment, ""))
    }

    // PYI059: `Generic[...]` entre las bases de una clase, pero no última. Avisa en los paréntesis.
    fn generic_not_last(self, node: Node<'_>) -> Option<Report> {
        if node.kind() != ARGUMENT_LIST {
            return None;
        }
        node.parent().filter(|class| {
            class.kind() == CLASS
                && class
                    .child_by_field_name(SUPERCLASSES_FIELD)
                    .is_some_and(|bases| bases.id() == node.id())
        })?;
        let bases: Vec<Node<'_>> = named(node)
            .into_iter()
            .filter(|base| {
                base.kind() != COMMENT && base.kind() != KEYWORD_ARGUMENT && !SPLATS.contains(&base.kind())
            })
            .collect();
        let generic = bases
            .iter()
            .position(|base| self.is_typing_subscript(*base, GENERIC))?;
        (generic.saturating_add(1) != bases.len()).then(|| Report::at(PythonRule::PYI059, node, ""))
    }
}
