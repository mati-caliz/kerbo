use tree_sitter::Node;

use super::java::{IDENTIFIER, text};
use super::java_receivers::declared_simple_type;
use super::{LintFinding, LintKind, finding};
use crate::rules::JavaRule;
use crate::syntax::{descendants, named_children};

const METHOD: &str = "method_declaration";
const EXPRESSION_STATEMENT: &str = "expression_statement";
const ASSIGNMENT: &str = "assignment_expression";
const UPDATE: &str = "update_expression";
const INVOCATION: &str = "method_invocation";
const RETURN: &str = "return_statement";
const THIS: &str = "this";
const MODIFIERS: &str = "modifiers";
const FORMAL_PARAMETER: &str = "formal_parameter";
const SPREAD_PARAMETER: &str = "spread_parameter";
const DEFERRED_CODE: &[&str] = &["lambda_expression", "class_body"];
const COMMENTS: &[&str] = &["line_comment", "block_comment"];
const NAME_FIELD: &str = "name";
const BODY_FIELD: &str = "body";
const PARAMETERS_FIELD: &str = "parameters";
const ARGUMENTS_FIELD: &str = "arguments";
const OBJECT_FIELD: &str = "object";
const LEFT_FIELD: &str = "left";
const RIGHT_FIELD: &str = "right";
const OPERATOR_FIELD: &str = "operator";
const PLAIN_ASSIGNMENT: &str = "=";
const STATIC: &str = "static";
const CLOSED_MODIFIERS: &[&str] = &["private", "final", STATIC];

// Lo que un método devuelve en todos sus `return`: el objeto que lo recibe o uno de sus parámetros.
#[derive(PartialEq, Eq)]
enum Returned {
    Receiver,
    Parameter(usize),
}

// Calca "Variable is already assigned to this value" cuando el valor sale de una llamada: IntelliJ
// infiere el contrato de un método del archivo que nadie puede redefinir y que siempre devuelve uno
// de sus parámetros (`lista = agregarSiCorresponde(lista, item)`) o `this`
// (`vista = vista.withJobs(...)`).
pub(super) fn reported(nodes: &[Node<'_>], source: &[u8]) -> Vec<LintFinding> {
    let methods: Vec<Node<'_>> = nodes
        .iter()
        .copied()
        .filter(|node| node.kind() == METHOD)
        .collect();
    nodes
        .iter()
        .filter(|node| node.kind() == EXPRESSION_STATEMENT)
        .filter_map(|statement| {
            let variable = reassigned_to_itself(*statement, &methods, source)?;
            Some(finding(
                LintKind::Java(JavaRule::AlreadyAssigned),
                *statement,
                text(variable, source),
                None,
            ))
        })
        .collect()
}

fn reassigned_to_itself<'tree>(
    statement: Node<'tree>,
    methods: &[Node<'_>],
    source: &[u8],
) -> Option<Node<'tree>> {
    let assignment = named_children(statement)
        .into_iter()
        .find(|child| child.kind() == ASSIGNMENT)?;
    let operator = assignment.child_by_field_name(OPERATOR_FIELD)?;
    let target = assignment.child_by_field_name(LEFT_FIELD)?;
    let call = assignment.child_by_field_name(RIGHT_FIELD)?;
    if text(operator, source) != PLAIN_ASSIGNMENT || target.kind() != IDENTIFIER || call.kind() != INVOCATION
    {
        return None;
    }
    let called = text(call.child_by_field_name(NAME_FIELD)?, source);
    let mut named = methods.iter().filter(|method| {
        method
            .child_by_field_name(NAME_FIELD)
            .is_some_and(|name| text(name, source) == called)
    });
    let method = *named.next()?;
    if named.next().is_some() {
        return None;
    }
    let variable = text(target, source);
    let is_variable = |node: &Node<'_>| node.kind() == IDENTIFIER && text(*node, source) == variable;
    let arguments = listed(call.child_by_field_name(ARGUMENTS_FIELD)?);
    let same = match (
        call.child_by_field_name(OBJECT_FIELD),
        always_returned(method, source)?,
    ) {
        (Some(receiver), Returned::Receiver) => {
            is_variable(&receiver)
                && declared_simple_type(receiver, variable, source).as_deref()
                    == declaring_class(method, source)
        }
        (None, Returned::Parameter(index)) => {
            arguments.len() == parameters_of(method).len() && arguments.get(index).is_some_and(is_variable)
        }
        _ => false,
    };
    same.then_some(target)
}

fn always_returned(method: Node<'_>, source: &[u8]) -> Option<Returned> {
    let modifiers: Vec<&str> = named_children(method)
        .into_iter()
        .filter(|child| child.kind() == MODIFIERS)
        .flat_map(|modifiers| text(modifiers, source).split_whitespace())
        .collect();
    if !CLOSED_MODIFIERS.iter().any(|closed| modifiers.contains(closed)) {
        return None;
    }
    let body = method.child_by_field_name(BODY_FIELD)?;
    let in_body = descendants(body);
    let mut returned = in_body
        .iter()
        .filter(|node| node.kind() == RETURN && !is_deferred(**node, body))
        .map(|statement| listed(*statement).into_iter().next());
    let first = returned.next()??;
    if !returned.all(|other| other.is_some_and(|other| text(other, source) == text(first, source))) {
        return None;
    }
    if first.kind() == THIS {
        return (!modifiers.contains(&STATIC)).then_some(Returned::Receiver);
    }
    let name = text(first, source);
    let index = parameters_of(method).iter().position(|parameter| {
        parameter.kind() == FORMAL_PARAMETER
            && parameter
                .child_by_field_name(NAME_FIELD)
                .is_some_and(|declared| text(declared, source) == name)
    })?;
    let is_written = in_body.iter().any(|node| match node.kind() {
        ASSIGNMENT => node
            .child_by_field_name(LEFT_FIELD)
            .is_some_and(|left| text(left, source) == name),
        UPDATE => named_children(*node)
            .iter()
            .any(|operand| text(*operand, source) == name),
        _ => false,
    });
    (first.kind() == IDENTIFIER && !is_written).then_some(Returned::Parameter(index))
}

fn parameters_of(method: Node<'_>) -> Vec<Node<'_>> {
    method
        .child_by_field_name(PARAMETERS_FIELD)
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .filter(|parameter| [FORMAL_PARAMETER, SPREAD_PARAMETER].contains(&parameter.kind()))
        .collect()
}

fn declaring_class<'source>(method: Node<'_>, source: &'source [u8]) -> Option<&'source str> {
    let declaration = method.parent()?.parent()?;
    declaration
        .child_by_field_name(NAME_FIELD)
        .map(|name| text(name, source))
}

// Un `return` dentro de una lambda o de una clase anónima no es del método.
fn is_deferred(node: Node<'_>, body: Node<'_>) -> bool {
    std::iter::successors(node.parent(), Node::parent)
        .take_while(|ancestor| ancestor.id() != body.id())
        .any(|ancestor| DEFERRED_CODE.contains(&ancestor.kind()))
}

fn listed(parent: Node<'_>) -> Vec<Node<'_>> {
    named_children(parent)
        .into_iter()
        .filter(|child| !COMMENTS.contains(&child.kind()))
        .collect()
}
