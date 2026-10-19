use tree_sitter::Node;

use super::number::{java_integer_value, numeric_value};
use super::{field_is, named_children, operator, text};

const LITERALS: &[&str] = &[
    "decimal_integer_literal",
    "hex_integer_literal",
    "octal_integer_literal",
    "binary_integer_literal",
    "decimal_floating_point_literal",
    "hex_floating_point_literal",
];
const IGNORED_VALUES: &[f64] = &[-1.0, 0.0, 1.0, 2.0];
const IGNORED_INTEGERS: &[i64] = &[-1, 0, 1, 2];
const INTEGER_LITERALS: &[&str] = &[
    "decimal_integer_literal",
    "hex_integer_literal",
    "octal_integer_literal",
    "binary_integer_literal",
];
const EXEMPT_CONTEXTS: &[&str] = &[
    "annotation",
    "marker_annotation",
    "annotation_type_element_declaration",
];
const PARENTHESIZED: &str = "parenthesized_expression";
const UNARY: &str = "unary_expression";
const SIGNS: &[&str] = &["-", "+"];
const MINUS: &str = "-";
const METHOD: &str = "method_declaration";
const HASH_CODE: &str = "hashCode";
const TYPE_BODIES: &[&str] = &[
    "class_body",
    "interface_body",
    "enum_body",
    "enum_body_declarations",
    "annotation_type_body",
];
const VARIABLE: &str = "variable_declarator";
const ENUM_CONSTANT: &str = "enum_constant";
const FIELD: &str = "field_declaration";
const CLASS_BODY: &str = "class_body";
const FIELD_OWNERS: &[&str] = &[
    "class_declaration",
    "record_declaration",
    "object_creation_expression",
];
const COMPACT_SOURCE: &str = "program";
const MODIFIERS: &str = "modifiers";
const FINAL: &str = "final";
const BLOCK_OWNERS: &[&str] = &[
    "object_creation_expression",
    "array_creation_expression",
    "class_declaration",
    "interface_declaration",
    "enum_declaration",
    "record_declaration",
    "annotation_type_declaration",
];
const CONSTANT_BLOCKS: &[&str] = &["interface_declaration", "annotation_type_declaration"];
const WAIVING_PARENTS: &[&str] = &[
    "parenthesized_expression",
    "argument_list",
    "method_invocation",
    "object_creation_expression",
    "array_creation_expression",
    "array_initializer",
    "cast_expression",
    "ternary_expression",
];
const WAIVING_BINARY_OPERATORS: &[&str] = &[
    "*", "/", "+", "-", "%", ">>", ">>>", "<<", ">=", ">", "<=", "<", "==", "!=", "^", "|", "&",
];
const WAIVING_UNARY_OPERATORS: &[&str] = &["+", "-", "~"];
const BINARY: &str = "binary_expression";
const METHOD_CALL: &str = "method_invocation";
const ASSIGNMENT: &str = "assignment_expression";
const PLAIN_ASSIGNMENT: &str = "=";

pub fn reported<'tree>(node: Node<'tree>, source: &[u8]) -> Option<Node<'tree>> {
    if !LITERALS.contains(&node.kind()) || has_ancestor(node, EXEMPT_CONTEXTS) {
        return None;
    }
    let sign = sign(node, source);
    let negative = sign.is_some_and(|(_, operator)| operator == MINUS);
    let literal = text(node, source);
    let ignored = if INTEGER_LITERALS.contains(&node.kind()) {
        java_integer_value(literal)
            .and_then(|value| {
                if negative {
                    value.checked_neg()
                } else {
                    Some(value)
                }
            })
            .is_some_and(|value| IGNORED_INTEGERS.contains(&value))
    } else {
        numeric_value(literal)
            .map(|value| if negative { -value } else { value })
            .is_some_and(|value| {
                IGNORED_VALUES
                    .iter()
                    .any(|&ignored| (value - ignored).abs() < f64::EPSILON)
            })
    };
    if ignored || is_in_hash_code(node, source) || is_field_declaration(node) {
        return None;
    }
    let constant = containing_constant(node, source);
    is_magic(node, constant, source).then(|| sign.map_or(node, |(unary, _)| unary))
}

fn sign<'tree, 'source>(node: Node<'tree>, source: &'source [u8]) -> Option<(Node<'tree>, &'source str)> {
    let mut parent = node.parent()?;
    while parent.kind() == PARENTHESIZED {
        parent = parent.parent()?;
    }
    let operator = operator(parent, source);
    (parent.kind() == UNARY && SIGNS.contains(&operator)).then_some((parent, operator))
}

fn ancestors(node: Node<'_>) -> impl Iterator<Item = Node<'_>> {
    std::iter::successors(node.parent(), Node::parent)
}

fn has_ancestor(node: Node<'_>, kinds: &[&str]) -> bool {
    ancestors(node).any(|ancestor| kinds.contains(&ancestor.kind()))
}

fn is_in_hash_code(node: Node<'_>, source: &[u8]) -> bool {
    ancestors(node)
        .find(|ancestor| ancestor.kind() == METHOD)
        .is_some_and(|method| {
            method
                .child_by_field_name("name")
                .is_some_and(|name| text(name, source) == HASH_CODE)
                && method
                    .child_by_field_name("parameters")
                    .is_some_and(|parameters| named_children(parameters).is_empty())
        })
}

fn is_field_declaration(node: Node<'_>) -> bool {
    let variable = ancestors(node)
        .take_while(|ancestor| !TYPE_BODIES.contains(&ancestor.kind()))
        .find(|ancestor| ancestor.kind() == VARIABLE);
    let Some(declaration) = variable.and_then(|variable| variable.parent()) else {
        return false;
    };
    let Some(holder) = declaration.parent() else {
        return false;
    };
    holder.kind() == COMPACT_SOURCE
        || (declaration.kind() == FIELD
            && holder.kind() == CLASS_BODY
            && holder
                .parent()
                .is_some_and(|owner| FIELD_OWNERS.contains(&owner.kind())))
}

fn containing_constant<'tree>(node: Node<'tree>, source: &[u8]) -> Option<Node<'tree>> {
    let definition = ancestors(node).find(|ancestor| [VARIABLE, ENUM_CONSTANT].contains(&ancestor.kind()))?;
    let is_constant = definition.kind() == ENUM_CONSTANT
        || is_in_constant_block(definition)
        || definition
            .parent()
            .is_some_and(|declaration| is_final(declaration, source));
    is_constant.then_some(definition)
}

fn is_in_constant_block(definition: Node<'_>) -> bool {
    ancestors(definition)
        .find(|ancestor| BLOCK_OWNERS.contains(&ancestor.kind()))
        .is_some_and(|owner| CONSTANT_BLOCKS.contains(&owner.kind()))
}

fn is_final(declaration: Node<'_>, source: &[u8]) -> bool {
    named_children(declaration)
        .into_iter()
        .filter(|child| child.kind() == MODIFIERS)
        .any(|modifiers| {
            (0..modifiers.child_count())
                .filter_map(|index| modifiers.child(index))
                .any(|modifier| text(modifier, source) == FINAL)
        })
}

fn is_magic(node: Node<'_>, constant: Option<Node<'_>>, source: &[u8]) -> bool {
    let Some(constant) = constant else {
        return true;
    };
    std::iter::successors(Some(node), Node::parent)
        .zip(ancestors(node))
        .take_while(|&(_, ancestor)| ancestor.id() != constant.id())
        .any(|(child, ancestor)| !waives(ancestor, child, source))
}

fn waives(ancestor: Node<'_>, child: Node<'_>, source: &[u8]) -> bool {
    let kind = ancestor.kind();
    let operator = operator(ancestor, source);
    if kind == METHOD_CALL && field_is(ancestor, "object", child) {
        return false;
    }
    WAIVING_PARENTS.contains(&kind)
        || (kind == BINARY && WAIVING_BINARY_OPERATORS.contains(&operator))
        || (kind == UNARY && WAIVING_UNARY_OPERATORS.contains(&operator))
        || (kind == ASSIGNMENT && operator == PLAIN_ASSIGNMENT)
}
