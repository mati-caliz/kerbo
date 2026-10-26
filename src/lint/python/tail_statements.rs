use tree_sitter::Node;

use super::Report;
use super::ast::{
    ASSIGNMENT, ATTRIBUTE, CALL, CLASS, COMMENT, FALSE, FUNCTION, FUNCTION_FIELD, IDENTIFIER, LAMBDA,
    LEFT_FIELD, NAME_FIELD, RIGHT_FIELD, SUBSCRIPT, TRUE, TYPE_FIELD, VALUE_FIELD, is_bytes, is_string,
    named, text, unparenthesized,
};
use super::branches::equivalent;
use super::codegen;
use super::known_types::{Known, is_binding, is_known};
use crate::fix::Fix;
use crate::rules::PythonRule;

const IF: &str = "if_statement";
const ASSERT: &str = "assert_statement";
const CONDITIONAL: &str = "conditional_expression";
const EXPRESSION_STATEMENT: &str = "expression_statement";
const NONLOCAL: &str = "nonlocal_statement";
const RAISE: &str = "raise_statement";
const EXCEPT_CLAUSES: [&str; 2] = ["except_clause", "except_group_clause"];
const COMPARISON: &str = "comparison_operator";
const YIELD: &str = "yield";
const KEYWORD_ARGUMENT: &str = "keyword_argument";
const AS_PATTERN: &str = "as_pattern";
const BOOLEAN_OPERATOR: &str = "boolean_operator";
const BODY_FIELD: &str = "body";
const CONDITION_FIELD: &str = "condition";
const CONSEQUENCE_FIELD: &str = "consequence";
const ALTERNATIVE_FIELD: &str = "alternative";
const ARGUMENTS_FIELD: &str = "arguments";
const ATTRIBUTE_FIELD: &str = "attribute";
const OBJECT_FIELD: &str = "object";
const OPERATORS_FIELD: &str = "operators";
const SUPERCLASSES_FIELD: &str = "superclasses";
const REMOVE: &str = "remove";
const IN: &str = "in";
const METACLASS: &str = "metaclass";
const TYPE: &str = "type";
const STOP_ITERATION: &str = "StopIteration";
const CLASS_CELL: &str = "__class__";
const ORDERINGS: [&str; 4] = ["<", "<=", ">", ">="];
const MESSAGE_LITERALS: [&str; 6] = ["integer", "float", TRUE, FALSE, "none", "ellipsis"];
const SCOPES: [&str; 3] = [FUNCTION, CLASS, LAMBDA];
const SELF: &str = "self";
const SIMPLE_ELEMENTS: [&str; 3] = ["string", "integer", "float"];

// Más chequeos sobre sentencias: RUF040, FURB136, FURB132, B032, UP050, PLE0117, PLR1708 y PLW0711.
pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    let mut found = Vec::new();
    for node in nodes {
        match node.kind() {
            ASSERT => found.extend(literal_assert_message(*node, source)),
            CONDITIONAL => found.extend(ternary_min_max(*node, source)),
            IF => found.extend(check_and_remove(*node, source)),
            ASSIGNMENT => found.extend(unintentional_annotation(*node, source)),
            CLASS => found.extend(metaclass_type(*node, source)),
            NONLOCAL => found.extend(nonlocal_without_binding(*node, source)),
            RAISE => found.extend(stop_iteration_in_generator(*node, source)),
            kind if EXCEPT_CLAUSES.contains(&kind) => found.extend(boolean_exception(*node)),
            _ => {}
        }
    }
    found
}

fn single_statement(body: Option<Node<'_>>) -> Option<Node<'_>> {
    let statements = significant(body?);
    match statements.as_slice() {
        [only] => Some(*only),
        _ => None,
    }
}

fn significant(node: Node<'_>) -> Vec<Node<'_>> {
    named(node)
        .into_iter()
        .filter(|child| child.kind() != COMMENT)
        .collect()
}

// RUF040: `assert x, 2`, con un mensaje que no es texto.
fn literal_assert_message(statement: Node<'_>, source: &[u8]) -> Option<Report> {
    let message = unparenthesized(*significant(statement).get(1)?);
    let literal =
        MESSAGE_LITERALS.contains(&message.kind()) || (is_string(message) && is_bytes(message, source));
    literal.then(|| Report::at(PythonRule::RUF040, message, ""))
}

fn ordering<'tree>(condition: Node<'tree>, source: &[u8]) -> Option<(Node<'tree>, Node<'tree>)> {
    let condition = unparenthesized(condition);
    if condition.kind() != COMPARISON {
        return None;
    }
    let mut cursor = condition.walk();
    let operators: Vec<&str> = condition
        .children_by_field_name(OPERATORS_FIELD, &mut cursor)
        .map(|operator| text(operator, source))
        .collect();
    match (significant(condition).as_slice(), operators.as_slice()) {
        ([left, right], [operator]) if ORDERINGS.contains(operator) => Some((*left, *right)),
        _ => None,
    }
}

// FURB136: `a if a > b else b`, que es `max(a, b)`.
fn ternary_min_max(conditional: Node<'_>, source: &[u8]) -> Option<Report> {
    let parts = significant(conditional);
    let [body, condition, orelse] = parts.as_slice() else {
        return None;
    };
    let (left, right) = ordering(*condition, source)?;
    let same = |first: Node<'_>, second: Node<'_>| equivalent(first, second, source);
    let selects = (same(*body, left) && same(*orelse, right)) || (same(*body, right) && same(*orelse, left));
    selects.then(|| Report::at(PythonRule::FURB136, conditional, ""))
}

// FURB132: `if x in s: s.remove(x)` sobre un conjunto, que es `s.discard(x)`.
fn check_and_remove(statement: Node<'_>, source: &[u8]) -> Option<Report> {
    let report = removed_from_set(statement, source)?;
    let condition = unparenthesized(statement.child_by_field_name(CONDITION_FIELD)?);
    let operands = significant(condition);
    let [element, set] = operands.as_slice() else {
        return None;
    };
    let replacement = format!(
        "{}.discard({})",
        text(*set, source),
        codegen::unparse(*element, codegen::COMMA, source)
    );
    Some(report.with_fix(Some(
        Fix::replace(statement, &replacement, source).marked_unsafe(),
    )))
}

fn removed_from_set(statement: Node<'_>, source: &[u8]) -> Option<Report> {
    if statement.child_by_field_name(ALTERNATIVE_FIELD).is_some() {
        return None;
    }
    let condition = unparenthesized(statement.child_by_field_name(CONDITION_FIELD)?);
    let mut cursor = condition.walk();
    let operators: Vec<&str> = condition
        .children_by_field_name(OPERATORS_FIELD, &mut cursor)
        .map(|operator| text(operator, source))
        .collect();
    let operands = significant(condition);
    let [element, set] = operands.as_slice() else {
        return None;
    };
    let simple_element = element.kind() == IDENTIFIER || SIMPLE_ELEMENTS.contains(&element.kind());
    if condition.kind() != COMPARISON
        || operators != [IN]
        || !simple_element
        || !is_known(*set, Known::Set, source)
    {
        return None;
    }
    let only = single_statement(statement.child_by_field_name(CONSEQUENCE_FIELD))
        .filter(|only| only.kind() == EXPRESSION_STATEMENT)?;
    let call = significant(only)
        .first()
        .copied()
        .filter(|call| call.kind() == CALL)?;
    let function = call
        .child_by_field_name(FUNCTION_FIELD)
        .filter(|function| function.kind() == ATTRIBUTE)?;
    let removes = function
        .child_by_field_name(ATTRIBUTE_FIELD)
        .is_some_and(|attribute| text(attribute, source) == REMOVE)
        && function
            .child_by_field_name(OBJECT_FIELD)
            .is_some_and(|object| equivalent(object, *set, source))
        && call
            .child_by_field_name(ARGUMENTS_FIELD)
            .is_some_and(|arguments| matches!(significant(arguments).as_slice(), [argument] if equivalent(*argument, *element, source)));
    removes.then(|| Report::at(PythonRule::FURB132, statement, ""))
}

// B032: `a["b"]: 1`, una anotación donde se quiso asignar.
fn unintentional_annotation(assignment: Node<'_>, source: &[u8]) -> Option<Report> {
    if assignment.child_by_field_name(RIGHT_FIELD).is_some()
        || assignment.child_by_field_name(TYPE_FIELD).is_none()
    {
        return None;
    }
    let target = assignment.child_by_field_name(LEFT_FIELD)?;
    let owner = match target.kind() {
        SUBSCRIPT => target.child_by_field_name(VALUE_FIELD),
        ATTRIBUTE => target
            .child_by_field_name(OBJECT_FIELD)
            .filter(|object| text(*object, source) != SELF),
        _ => None,
    }?;
    (owner.kind() == IDENTIFIER).then(|| Report::at(PythonRule::B032, assignment, ""))
}

// UP050: `class A(metaclass=type)`, que es la metaclase de siempre.
fn metaclass_type(class: Node<'_>, source: &[u8]) -> Vec<Report> {
    class
        .child_by_field_name(SUPERCLASSES_FIELD)
        .map(significant)
        .unwrap_or_default()
        .into_iter()
        .filter(|argument| argument.kind() == KEYWORD_ARGUMENT)
        .filter(|argument| {
            argument
                .child_by_field_name(NAME_FIELD)
                .is_some_and(|name| text(name, source) == METACLASS)
                && argument
                    .child_by_field_name(VALUE_FIELD)
                    .is_some_and(|value| text(value, source) == TYPE)
        })
        .map(|argument| Report::at(PythonRule::UP050, argument, ""))
        .collect()
}

// PLE0117: `nonlocal x` sin una función de afuera que ligue `x`. Avisa en el nombre; `__class__` es
// la celda implícita de los métodos.
fn nonlocal_without_binding(statement: Node<'_>, source: &[u8]) -> Vec<Report> {
    let current = std::iter::successors(statement.parent(), Node::parent)
        .find(|ancestor| matches!(ancestor.kind(), FUNCTION | CLASS));
    let enclosing: Vec<Node<'_>> =
        std::iter::successors(current.and_then(|scope| scope.parent()), Node::parent)
            .filter(|ancestor| ancestor.kind() == FUNCTION)
            .collect();
    significant(statement)
        .into_iter()
        .filter(|name| name.kind() == IDENTIFIER && text(*name, source) != CLASS_CELL)
        .filter(|name| {
            let in_class = current.is_some_and(|scope| scope.kind() == CLASS);
            !enclosing.iter().any(|function| {
                binds_in_function(
                    *function,
                    text(*name, source),
                    source,
                    in_class.then(|| name.start_byte()),
                )
            })
        })
        .map(|name| Report::at(PythonRule::PLE0117, name, ""))
        .collect()
}

fn binds_in_function(function: Node<'_>, wanted: &str, source: &[u8], before: Option<usize>) -> bool {
    let mut pending: Vec<Node<'_>> = named(function)
        .into_iter()
        .filter(|child| child.kind() != IDENTIFIER)
        .collect();
    while let Some(node) = pending.pop() {
        if node.kind() == IDENTIFIER {
            let declared = node
                .parent()
                .is_some_and(|parent| parent.kind() == NONLOCAL || parent.kind() == "global_statement");
            if text(node, source) == wanted
                && is_binding(node)
                && !declared
                && before.is_none_or(|limit| node.start_byte() < limit)
            {
                return true;
            }
            continue;
        }
        if SCOPES.contains(&node.kind()) {
            if node
                .child_by_field_name(NAME_FIELD)
                .is_some_and(|name| text(name, source) == wanted)
            {
                return true;
            }
            continue;
        }
        pending.extend(named(node));
    }
    false
}

// PLR1708: `raise StopIteration` en un generador, que Python convierte en `RuntimeError`.
fn stop_iteration_in_generator(statement: Node<'_>, source: &[u8]) -> Option<Report> {
    let exception = unparenthesized(*significant(statement).first()?);
    let raised = if exception.kind() == CALL {
        exception.child_by_field_name(FUNCTION_FIELD)?
    } else {
        exception
    };
    if raised.kind() != IDENTIFIER || text(raised, source) != STOP_ITERATION {
        return None;
    }
    let function = std::iter::successors(statement.parent(), Node::parent)
        .find(|ancestor| SCOPES.contains(&ancestor.kind()))
        .filter(|scope| scope.kind() == FUNCTION)?;
    contains_yield(function).then(|| Report::at(PythonRule::PLR1708, statement, ""))
}

fn contains_yield(function: Node<'_>) -> bool {
    let mut pending = function
        .child_by_field_name(BODY_FIELD)
        .map(named)
        .unwrap_or_default();
    while let Some(node) = pending.pop() {
        if node.kind() == YIELD {
            return true;
        }
        if !SCOPES.contains(&node.kind()) {
            pending.extend(named(node));
        }
    }
    false
}

// PLW0711: `except A or B:`, que sólo atrapa `A`.
fn boolean_exception(handler: Node<'_>) -> Option<Report> {
    let value = handler.child_by_field_name(VALUE_FIELD)?;
    let caught = if value.kind() == AS_PATTERN {
        *significant(value).first()?
    } else {
        value
    };
    let caught = unparenthesized(caught);
    (caught.kind() == BOOLEAN_OPERATOR).then(|| Report::at(PythonRule::PLW0711, caught, ""))
}
