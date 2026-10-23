use std::collections::HashMap;

use tree_sitter::Node;

use super::Context;
use super::nodes::{
    ARGUMENTS, ARGUMENTS_FIELD, ASSIGNMENT, AUGMENTED_ASSIGNMENT, BINARY, CALL, COMMENT, FUNCTION_FIELD,
    IDENTIFIER, LEFT_FIELD, NUMBER, RIGHT_FIELD, STRING, TEMPLATE_STRING, TEMPLATE_SUBSTITUTION,
    first_named_child, operator, without_parentheses,
};
use super::scope::ScopeTree;
use super::strings::{cooked, numeric_text, static_string};
use super::tokens::{AND, OR};
use crate::syntax::named_children;

pub const NULLISH: &str = "??";
pub const UNDEFINED: &str = "undefined";
pub const UNARY: &str = "unary_expression";
pub const SEQUENCE: &str = "sequence_expression";
pub const NEW: &str = "new_expression";
pub const ARRAY: &str = "array";
pub const SPREAD: &str = "spread_element";
pub const TRUE: &str = "true";
pub const FALSE: &str = "false";
pub const NULL: &str = "null";
pub const REGEX: &str = "regex";
pub const LITERALS: &[&str] = &[STRING, NUMBER, REGEX, TRUE, FALSE, NULL];
pub const FUNCTIONS: &[&str] = &[
    "arrow_function",
    "function_expression",
    "function",
    "generator_function",
];
pub const OBJECTS: &[&str] = &["object", "class"];
pub const VOID: &str = "void";
pub const TYPEOF: &str = "typeof";
pub const NOT: &str = "!";
pub const BOOLEAN: &str = "Boolean";
const OPTIONAL_CHAIN_FIELD: &str = "optional_chain";
const ARGUMENT_FIELD: &str = "argument";
const STRING_FRAGMENT: &str = "string_fragment";
const ESCAPE_SEQUENCE: &str = "escape_sequence";
const LOGICAL_ASSIGNMENTS: &[(&str, &str)] = &[("||=", OR), ("&&=", AND)];

// La vista de ESTree sobre el árbol de tree-sitter que necesitan `isConstant` y las reglas que
// lo usan: sin paréntesis, con `&&`, `||` y `??` como expresiones lógicas aparte.
pub struct Constants<'context, 'tree, 'source> {
    context: &'context Context<'tree, 'source>,
    references: HashMap<usize, Option<usize>>,
    tree: &'context ScopeTree<'tree>,
}

impl<'context, 'tree, 'source> Constants<'context, 'tree, 'source> {
    pub fn new(context: &'context Context<'tree, 'source>) -> Self {
        let tree = context.scopes();
        let references = tree
            .references
            .iter()
            .map(|reference| (reference.identifier.id(), reference.resolved))
            .collect();
        Self {
            context,
            references,
            tree,
        }
    }

    pub fn text(&self, node: Node<'_>) -> &'source str {
        node.utf8_text(self.context.source).unwrap_or_default()
    }

    pub fn operator(&self, node: Node<'_>) -> &'source str {
        operator(node, self.context.source)
    }

    // El tipo de ESTree que importa: un `binary_expression` con `&&`, `||` o `??` es lógico.
    pub fn is_logical(&self, node: Node<'_>) -> bool {
        node.kind() == BINARY && [AND, OR, NULLISH].contains(&self.operator(node))
    }

    pub fn is_binary(&self, node: Node<'_>) -> bool {
        node.kind() == BINARY && !self.is_logical(node)
    }

    // `isReferenceToGlobalVariable`: el nombre no tiene una declaración propia que lo tape.
    pub fn is_global(&self, identifier: Node<'_>) -> bool {
        self.references.get(&identifier.id()).is_some_and(|resolved| {
            resolved
                .and_then(|id| self.tree.variable(id))
                .is_none_or(|variable| variable.definitions.is_empty())
        })
    }

    pub fn is_global_undefined(&self, node: Node<'_>) -> bool {
        [IDENTIFIER, UNDEFINED].contains(&node.kind()) && self.text(node) == UNDEFINED && self.is_global(node)
    }

    // La función llamada por nombre (`Boolean(x)`), si no es una llamada opcional ni con template.
    pub fn called_global(&self, call: Node<'tree>) -> Option<(&'source str, Vec<Node<'tree>>)> {
        if call.kind() != CALL || call.child_by_field_name(OPTIONAL_CHAIN_FIELD).is_some() {
            return None;
        }
        let callee = without_parentheses(call.child_by_field_name(FUNCTION_FIELD)?);
        let arguments = call
            .child_by_field_name(ARGUMENTS_FIELD)
            .filter(|arguments| arguments.kind() == ARGUMENTS)?;
        (callee.kind() == IDENTIFIER && self.is_global(callee))
            .then(|| (self.text(callee), values(arguments)))
    }

    pub fn is_boolean_call(&self, node: Node<'tree>) -> bool {
        self.called_global(node).is_some_and(|(name, arguments)| {
            name == BOOLEAN
                && arguments
                    .first()
                    .is_none_or(|first| self.is_constant(*first, true))
        })
    }

    pub fn is_constant(&self, node: Node<'tree>, in_boolean_position: bool) -> bool {
        let node = without_parentheses(node);
        let kind = node.kind();
        if LITERALS.contains(&kind) || FUNCTIONS.contains(&kind) || OBJECTS.contains(&kind) {
            return true;
        }
        if self.is_logical(node) {
            return self.is_constant_logical(node, in_boolean_position);
        }
        match kind {
            TEMPLATE_STRING => {
                (in_boolean_position && has_cooked_text(node, self.context.source))
                    || substitutions(node).all(|expression| self.is_constant(expression, false))
            }
            ARRAY => {
                in_boolean_position
                    || values(node)
                        .into_iter()
                        .all(|element| self.is_constant(element, false))
            }
            UNARY => self.is_constant_unary(node, in_boolean_position),
            BINARY => {
                self.operator(node) != "in"
                    && side(node, LEFT_FIELD).is_none_or(|left| self.is_constant(left, false))
                    && side(node, RIGHT_FIELD).is_none_or(|right| self.is_constant(right, false))
            }
            NEW => in_boolean_position,
            ASSIGNMENT => {
                side(node, RIGHT_FIELD).is_some_and(|right| self.is_constant(right, in_boolean_position))
            }
            AUGMENTED_ASSIGNMENT => {
                in_boolean_position
                    && logical_operator_of(self.operator(node)).is_some_and(|logical| {
                        side(node, RIGHT_FIELD).is_some_and(|right| self.is_logical_identity(right, logical))
                    })
            }
            SEQUENCE => last_expression(node).is_some_and(|last| self.is_constant(last, in_boolean_position)),
            SPREAD => first_named_child(node)
                .is_some_and(|argument| self.is_constant(argument, in_boolean_position)),
            CALL => self.is_boolean_call(node),
            IDENTIFIER | UNDEFINED => self.is_global_undefined(node),
            _ => false,
        }
    }

    fn is_constant_unary(&self, node: Node<'tree>, in_boolean_position: bool) -> bool {
        let operator = self.operator(node);
        if operator == VOID || (operator == TYPEOF && in_boolean_position) {
            return true;
        }
        side(node, ARGUMENT_FIELD).is_some_and(|argument| self.is_constant(argument, operator == NOT))
    }

    fn is_constant_logical(&self, node: Node<'tree>, in_boolean_position: bool) -> bool {
        let operator = self.operator(node);
        let (Some(left), Some(right)) = (side(node, LEFT_FIELD), side(node, RIGHT_FIELD)) else {
            return false;
        };
        let left_constant = self.is_constant(left, in_boolean_position);
        let right_constant = self.is_constant(right, in_boolean_position);
        (left_constant && (right_constant || self.is_logical_identity(left, operator)))
            || (in_boolean_position && right_constant && self.is_logical_identity(right, operator))
    }

    // `isLogicalIdentity`: el valor que decide solo un `||` (algo verdadero) o un `&&` (algo falso).
    pub fn is_logical_identity(&self, node: Node<'tree>, operator: &str) -> bool {
        let node = without_parentheses(node);
        if LITERALS.contains(&node.kind()) {
            return self
                .boolean_value(node)
                .is_some_and(|value| (operator == OR && value) || (operator == AND && !value));
        }
        if self.is_logical(node) {
            return self.operator(node) == operator
                && [LEFT_FIELD, RIGHT_FIELD]
                    .iter()
                    .filter_map(|field| side(node, field))
                    .any(|operand| self.is_logical_identity(operand, operator));
        }
        match node.kind() {
            UNARY => operator == AND && self.operator(node) == VOID,
            AUGMENTED_ASSIGNMENT => {
                logical_operator_of(self.operator(node)) == Some(operator)
                    && side(node, RIGHT_FIELD).is_some_and(|right| self.is_logical_identity(right, operator))
            }
            _ => false,
        }
    }

    // `getBooleanValue` de un literal.
    fn boolean_value(&self, literal: Node<'_>) -> Option<bool> {
        match literal.kind() {
            TRUE | REGEX => Some(true),
            FALSE | NULL => Some(false),
            STRING => static_string(literal, self.context.source).map(|value| !value.is_empty()),
            NUMBER => Some(numeric_text(self.text(literal)).is_none_or(|value| value != "0")),
            _ => None,
        }
    }
}

pub fn side<'tree>(node: Node<'tree>, field: &str) -> Option<Node<'tree>> {
    node.child_by_field_name(field).map(without_parentheses)
}

fn logical_operator_of(assignment: &str) -> Option<&'static str> {
    LOGICAL_ASSIGNMENTS
        .iter()
        .find(|(candidate, _)| *candidate == assignment)
        .map(|(_, logical)| *logical)
}

pub fn values(node: Node<'_>) -> Vec<Node<'_>> {
    named_children(node)
        .into_iter()
        .filter(|child| child.kind() != COMMENT)
        .collect()
}

pub fn last_expression(sequence: Node<'_>) -> Option<Node<'_>> {
    values(sequence).last().copied()
}

pub fn substitutions(template: Node<'_>) -> impl Iterator<Item = Node<'_>> {
    named_children(template)
        .into_iter()
        .filter(|child| child.kind() == TEMPLATE_SUBSTITUTION)
        .filter_map(first_named_child)
}

fn has_cooked_text(template: Node<'_>, source: &[u8]) -> bool {
    named_children(template)
        .into_iter()
        .any(|child| match child.kind() {
            STRING_FRAGMENT => true,
            ESCAPE_SEQUENCE => !cooked(child.utf8_text(source).unwrap_or_default()).is_empty(),
            _ => false,
        })
}
