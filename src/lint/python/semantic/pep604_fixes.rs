use tree_sitter::Node;

use super::pep::flattened;
use super::qualified::is_typing;
use crate::lint::python::ast::{COMMENT, SUBSCRIPT_FIELD, TUPLE, named, text, unparenthesized};
use crate::lint::python::codegen::{BIT_OR, unparse};
use crate::rules::PythonRule;

const OPTIONAL: &str = "Optional";
const NONE: &str = "none";
const NONE_TEXT: &str = "None";
const TYPE_NODE: &str = "type";
const SUBSCRIPT: &str = "subscript";
const GENERIC_TYPE: &str = "generic_type";
const BINARY_OPERATOR: &str = "binary_operator";
const UNION_TYPE: &str = "union_type";
const SLICE: &str = "slice";
const BIT_OR_TEXT: &str = "|";
const OPERATOR_FIELD: &str = "operator";
const VALUE_FIELD: &str = "value";
const LEFT_FIELD: &str = "left";
const RIGHT_FIELD: &str = "right";
const SEPARATOR: &str = " | ";
const DISALLOWED: [&str; 7] = [
    "named_expression",
    "await",
    "lambda",
    "yield",
    "list_splat",
    "list_splat_pattern",
    SLICE,
];

// El texto del arreglo de UP045 y UP007 de ruff: `Optional[X]` pasa a `X | None` (sacando los
// `Optional` anidados y los `None` de una unión de adentro) y `Union[X, Y]` a `X | Y`, impresos con el
// generador. Quien llama lo ubica y lo vuelve inseguro si hay comentarios.
pub fn pep604_replacement(
    rule: PythonRule,
    slices: &[Node<'_>],
    qualify: &dyn Fn(Node<'_>) -> Option<String>,
    source: &[u8],
) -> Option<String> {
    if rule == PythonRule::UP045 {
        optional_replacement(slices, qualify, source)
    } else {
        union_replacement(slices, source)
    }
}

fn optional_replacement(
    slices: &[Node<'_>],
    qualify: &dyn Fn(Node<'_>) -> Option<String>,
    source: &[u8],
) -> Option<String> {
    let [slice] = slices else {
        return None;
    };
    let slice = unwrap(*slice);
    if slice.kind() == TUPLE || slice.kind() == NONE || !is_allowed(slice) {
        return None;
    }
    let mut inner = slice;
    while let Some((value, [only])) = parts(inner)
        .as_ref()
        .map(|(value, indexes)| (*value, indexes.as_slice()))
    {
        if !qualify(value).is_some_and(|name| is_typing(&name, OPTIONAL)) {
            break;
        }
        inner = unwrap(*only);
    }
    let elements = if is_bit_or(inner, source) {
        let mut elements = Vec::new();
        collect_non_none(inner, source, &mut elements);
        if elements.is_empty() {
            return None;
        }
        elements
    } else {
        vec![inner]
    };
    Some(format!("{}{SEPARATOR}{NONE_TEXT}", union(&elements, source)))
}

fn union_replacement(slices: &[Node<'_>], source: &[u8]) -> Option<String> {
    let elements: Vec<Node<'_>> = slices.iter().flat_map(|slice| flattened(*slice)).collect();
    let single = match slices {
        [only] => Some(unwrap(*only)).filter(|only| only.kind() != TUPLE),
        _ => None,
    };
    if let Some(only) = single {
        if only.kind() == SLICE || !is_allowed(only) {
            return None;
        }
        let inner = text(only, source);
        return Some(if inner.contains('\n') {
            format!("({inner})")
        } else {
            inner.to_owned()
        });
    }
    elements
        .iter()
        .all(|element| is_allowed(*element))
        .then(|| union(&elements, source))
}

// `pep_604_union`: el primero al nivel de `|` y los demás uno más arriba, como una cadena a izquierda.
fn union(elements: &[Node<'_>], source: &[u8]) -> String {
    elements
        .iter()
        .enumerate()
        .map(|(index, element)| {
            let level = if index == 0 {
                BIT_OR
            } else {
                BIT_OR.saturating_add(1)
            };
            unparse(*element, level, source)
        })
        .collect::<Vec<_>>()
        .join(SEPARATOR)
}

fn collect_non_none<'tree>(node: Node<'tree>, source: &[u8], elements: &mut Vec<Node<'tree>>) {
    let node = unwrap(node);
    if is_bit_or(node, source) {
        for side in sides(node) {
            collect_non_none(side, source, elements);
        }
    } else if node.kind() != NONE {
        elements.push(node);
    }
}

fn is_bit_or(node: Node<'_>, source: &[u8]) -> bool {
    node.kind() == UNION_TYPE
        || (node.kind() == BINARY_OPERATOR
            && node
                .child_by_field_name(OPERATOR_FIELD)
                .is_some_and(|operator| text(operator, source) == BIT_OR_TEXT))
}

fn sides(node: Node<'_>) -> Vec<Node<'_>> {
    if node.kind() == UNION_TYPE {
        return named(node)
            .into_iter()
            .filter(|child| child.kind() != COMMENT)
            .collect();
    }
    [LEFT_FIELD, RIGHT_FIELD]
        .into_iter()
        .filter_map(|field| node.child_by_field_name(field))
        .collect()
}

// El valor y los índices de `X[...]`, como subíndice o como tipo genérico.
fn parts(node: Node<'_>) -> Option<(Node<'_>, Vec<Node<'_>>)> {
    match node.kind() {
        SUBSCRIPT => {
            let mut cursor = node.walk();
            let slices = node
                .children_by_field_name(SUBSCRIPT_FIELD, &mut cursor)
                .collect();
            Some((node.child_by_field_name(VALUE_FIELD)?, slices))
        }
        GENERIC_TYPE => {
            let children: Vec<Node<'_>> = named(node)
                .into_iter()
                .filter(|child| child.kind() != COMMENT)
                .collect();
            let (value, rest) = children.split_first()?;
            let slices = rest.iter().flat_map(|parameters| named(*parameters)).collect();
            Some((*value, slices))
        }
        _ => None,
    }
}

fn unwrap(node: Node<'_>) -> Node<'_> {
    let mut current = unparenthesized(node);
    while current.kind() == TYPE_NODE {
        let Some(inner) = named(current).into_iter().find(|child| child.kind() != COMMENT) else {
            break;
        };
        current = unparenthesized(inner);
    }
    current
}

// `is_allowed_value` de ruff: lo que puede ir de un lado de un `|` sin cambiar de sentido.
fn is_allowed(node: Node<'_>) -> bool {
    let node = unwrap(node);
    if node.kind() == TUPLE {
        return named(node)
            .into_iter()
            .filter(|child| child.kind() != COMMENT)
            .all(is_allowed);
    }
    !DISALLOWED.contains(&node.kind())
}
