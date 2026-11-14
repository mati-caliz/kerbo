use tree_sitter::Node;

use super::java::text;
use super::java_null_inference::{CallContext, NullArgument, argument_of_call};
use super::java_null_origin::NullReturn;
use super::java_owner::null_marking_of;
use super::java_receivers::simple_type_name;
use super::java_type_arguments::{TypeArgument, type_arguments_of};
use crate::syntax::{named_children, one_based};

const LAMBDA: &str = "lambda_expression";
const CLASS_BODY: &str = "class_body";
const BLOCK: &str = "block";
const RETURN: &str = "return_statement";
const NULL: &str = "null_literal";
const PARENTHESIZED: &str = "parenthesized_expression";
const ARGUMENT_LIST: &str = "argument_list";
const VARIABLE_DECLARATOR: &str = "variable_declarator";
const ASSIGNMENT: &str = "assignment_expression";
const BODY_FIELD: &str = "body";
const TYPE_FIELD: &str = "type";
const VALUE_FIELD: &str = "value";
const NULL_TEXT: &str = "null";
const COMMENTS: &[&str] = &["line_comment", "block_comment"];

// Una lambda que devuelve un `null` escrito y adónde va: el argumento de una llamada (y si la
// llamada tiene un tipo destino, que decide las variables de tipo del método) o una variable
// declarada con su interfaz funcional.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LambdaReturn {
    pub returns: Vec<NullReturn>,
    pub target: LambdaTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LambdaTarget {
    Argument {
        argument: NullArgument,
        target_typed: bool,
    },
    Declared {
        type_name: String,
        arguments: Vec<TypeArgument>,
    },
}

pub(super) fn lambda_returns_in(
    nodes: &[Node<'_>],
    source: &[u8],
    context: &CallContext<'_>,
) -> Vec<LambdaReturn> {
    nodes
        .iter()
        .filter(|node| node.kind() == LAMBDA)
        .filter_map(|lambda| {
            let returns = null_returns(*lambda, source);
            if returns.is_empty() {
                return None;
            }
            let target = target_of(*lambda, source, context)?;
            Some(LambdaReturn { returns, target })
        })
        .collect()
}

fn null_returns(lambda: Node<'_>, source: &[u8]) -> Vec<NullReturn> {
    let Some(body) = lambda.child_by_field_name(BODY_FIELD) else {
        return Vec::new();
    };
    let literals = if body.kind() == BLOCK {
        returned_literals(body)
    } else {
        Some(unparenthesized(body))
            .filter(|value| value.kind() == NULL)
            .into_iter()
            .collect()
    };
    let marking = null_marking_of(lambda, source);
    literals
        .into_iter()
        .map(|literal| {
            let start = literal.start_position();
            NullReturn {
                name: NULL_TEXT.to_owned(),
                line: one_based(start.row),
                column: one_based(start.column),
                marking,
            }
        })
        .collect()
}

// Los `null` que devuelven los `return` del cuerpo, sin entrar a otras lambdas ni clases.
fn returned_literals(body: Node<'_>) -> Vec<Node<'_>> {
    let mut found = Vec::new();
    let mut pending = vec![body];
    while let Some(node) = pending.pop() {
        if node.kind() == RETURN {
            found.extend(
                named_children(node)
                    .into_iter()
                    .find(|child| !COMMENTS.contains(&child.kind()))
                    .map(unparenthesized)
                    .filter(|value| value.kind() == NULL),
            );
        } else if node.kind() != LAMBDA && node.kind() != CLASS_BODY {
            pending.extend(named_children(node));
        }
    }
    found
}

fn target_of(lambda: Node<'_>, source: &[u8], context: &CallContext<'_>) -> Option<LambdaTarget> {
    let written = outermost_parentheses(lambda);
    let parent = written.parent()?;
    match parent.kind() {
        ARGUMENT_LIST => {
            let index = named_children(parent)
                .into_iter()
                .filter(|argument| !COMMENTS.contains(&argument.kind()))
                .position(|argument| argument.id() == written.id())?;
            let call = parent.parent()?;
            let argument = argument_of_call(call, (index, lambda), source, context)?;
            Some(LambdaTarget::Argument {
                argument,
                target_typed: has_target_type(call),
            })
        }
        VARIABLE_DECLARATOR
            if parent
                .child_by_field_name(VALUE_FIELD)
                .is_some_and(|value| value.id() == written.id()) =>
        {
            let declared = parent.parent()?.child_by_field_name(TYPE_FIELD)?;
            Some(LambdaTarget::Declared {
                type_name: simple_type_name(text(declared, source)),
                arguments: type_arguments_of(declared, source),
            })
        }
        _ => None,
    }
}

// Si lo que devuelve la llamada se asigna a algo con un tipo escrito: entonces las variables de
// tipo del método salen de ese tipo y no de los argumentos.
fn has_target_type(call: Node<'_>) -> bool {
    outermost_parentheses(call)
        .parent()
        .is_some_and(|parent| parent.kind() == VARIABLE_DECLARATOR || parent.kind() == ASSIGNMENT)
}

fn outermost_parentheses(node: Node<'_>) -> Node<'_> {
    let mut current = node;
    while let Some(parent) = current.parent().filter(|parent| parent.kind() == PARENTHESIZED) {
        current = parent;
    }
    current
}

fn unparenthesized(node: Node<'_>) -> Node<'_> {
    let mut current = node;
    while current.kind() == PARENTHESIZED {
        match named_children(current).into_iter().next() {
            Some(inner) => current = inner,
            None => break,
        }
    }
    current
}
