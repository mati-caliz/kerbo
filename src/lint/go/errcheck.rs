use tree_sitter::{Node, Point};

use super::parentheses::without_parentheses;
use crate::syntax::named_children;

const EXPRESSION_STATEMENT: &str = "expression_statement";
const RECEIVE_STATEMENT: &str = "receive_statement";
const GO_STATEMENT: &str = "go_statement";
const DEFER_STATEMENT: &str = "defer_statement";
const FOR_STATEMENT: &str = "for_statement";
const RANGE_CLAUSE: &str = "range_clause";
const CALL: &str = "call_expression";
const UNARY: &str = "unary_expression";
const SELECTOR: &str = "selector_expression";
const IDENTIFIER: &str = "identifier";
const INDEX: &str = "index_expression";
const PARENTHESIZED: &str = "parenthesized_expression";
const TYPE_ASSERTION: &str = "type_assertion_expression";
const ASSIGNMENTS: &[&str] = &["assignment_statement", "short_var_declaration"];
const VAR_DECLARATION: &str = "var_declaration";
const VAR_SPEC: &str = "var_spec";
const COMMENT: &str = "comment";
const RECEIVE: &str = "<-";
const SELECTOR_SEPARATOR: &str = ".";
const ARGUMENTS_FIELD: &str = "arguments";
const FUNCTION_FIELD: &str = "function";
const OPERATOR_FIELD: &str = "operator";
const OPERAND_FIELD: &str = "operand";
const FIELD_FIELD: &str = "field";
const RIGHT_FIELD: &str = "right";
const VALUE_FIELD: &str = "value";
const LEFT_FIELD: &str = "left";
const BOUND_VALUES: usize = 2;

// Dónde errcheck mira un error que nadie revisa, sin saber todavía los tipos: el paréntesis de una
// llamada suelta, con `go` o con `defer`, el `<-` de una recepción suelta y el `for` de un `range`
// al que le falta la clave o el valor. Los tipos confirman cada uno al final del escaneo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub position: Point,
    pub subject: String,
}

pub fn candidates(root: Node<'_>, source: &[u8]) -> Vec<Candidate> {
    let mut found = Vec::new();
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if skips_its_children(node) {
            continue;
        }
        found.extend(candidate(node, source));
        let mut children = named_children(node);
        children.reverse();
        pending.extend(children);
    }
    found
}

// Como el visitor de errcheck, que no entra en una aserción de tipo ni en una asignación (o un
// `var`) cuyo único valor es una.
fn skips_its_children(node: Node<'_>) -> bool {
    match node.kind() {
        TYPE_ASSERTION => true,
        kind if ASSIGNMENTS.contains(&kind) => single_type_assertion(node.child_by_field_name(RIGHT_FIELD)),
        VAR_DECLARATION => crate::syntax::descendants(node)
            .into_iter()
            .filter(|spec| spec.kind() == VAR_SPEC)
            .any(|spec| single_type_assertion(spec.child_by_field_name(VALUE_FIELD))),
        _ => false,
    }
}

fn single_type_assertion(values: Option<Node<'_>>) -> bool {
    values.is_some_and(|values| {
        let expressions: Vec<Node<'_>> = named_children(values)
            .into_iter()
            .filter(|value| value.kind() != COMMENT)
            .collect();
        matches!(expressions.as_slice(), [only] if only.kind() == TYPE_ASSERTION)
    })
}

fn candidate(node: Node<'_>, source: &[u8]) -> Option<Candidate> {
    match node.kind() {
        EXPRESSION_STATEMENT => {
            let expression = first_expression(node)?;
            if expression.kind() == CALL {
                return call_candidate(expression, source);
            }
            receive_candidate(expression, source)
        }
        RECEIVE_STATEMENT if node.child_by_field_name(LEFT_FIELD).is_none() => {
            receive_candidate(node.child_by_field_name(RIGHT_FIELD)?, source)
        }
        GO_STATEMENT | DEFER_STATEMENT => call_candidate(first_expression(node)?, source),
        FOR_STATEMENT => {
            let clause = named_children(node)
                .into_iter()
                .find(|child| child.kind() == RANGE_CLAUSE)?;
            let bound = clause
                .child_by_field_name(LEFT_FIELD)
                .map_or(0, |left| named_children(left).len());
            (bound < BOUND_VALUES).then(|| Candidate {
                position: node.start_position(),
                subject: String::new(),
            })
        }
        _ => None,
    }
}

// `<-canal` suelto, también como `case <-canal:` de un `select`, que go/ast escribe igual.
fn receive_candidate(expression: Node<'_>, source: &[u8]) -> Option<Candidate> {
    let unary = without_parentheses(expression);
    let operator = unary.child_by_field_name(OPERATOR_FIELD)?;
    (unary.kind() == UNARY && operator.utf8_text(source).ok()? == RECEIVE).then(|| Candidate {
        position: operator.start_position(),
        subject: String::new(),
    })
}

fn first_expression(node: Node<'_>) -> Option<Node<'_>> {
    named_children(node)
        .into_iter()
        .find(|child| child.kind() != COMMENT)
}

fn call_candidate(call: Node<'_>, source: &[u8]) -> Option<Candidate> {
    if call.kind() != CALL {
        return None;
    }
    let arguments = call.child_by_field_name(ARGUMENTS_FIELD)?;
    let subject = call
        .child_by_field_name(FUNCTION_FIELD)
        .and_then(|function| selector_name(base_function(function), source))
        .unwrap_or_default();
    Some(Candidate {
        position: arguments.start_position(),
        subject,
    })
}

// `baseCallExpr` de errcheck: sin índices de tipos genéricos ni paréntesis.
fn base_function(mut function: Node<'_>) -> Node<'_> {
    while matches!(function.kind(), INDEX | PARENTHESIZED) {
        let inner = if function.kind() == INDEX {
            function.child_by_field_name(OPERAND_FIELD)
        } else {
            first_expression(function)
        };
        let Some(inner) = inner else {
            break;
        };
        function = inner;
    }
    function
}

// `getSelectorName`: `a.b.c` si es una cadena de nombres.
fn selector_name(selector: Node<'_>, source: &[u8]) -> Option<String> {
    if selector.kind() != SELECTOR {
        return None;
    }
    let operand = selector.child_by_field_name(OPERAND_FIELD)?;
    let field = selector
        .child_by_field_name(FIELD_FIELD)?
        .utf8_text(source)
        .ok()?;
    let prefix = match operand.kind() {
        IDENTIFIER => operand.utf8_text(source).ok()?.to_owned(),
        _ => selector_name(operand, source)?,
    };
    Some([prefix.as_str(), field].join(SELECTOR_SEPARATOR))
}
