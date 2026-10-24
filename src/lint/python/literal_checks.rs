use tree_sitter::Node;

use super::Report;
use super::ast::{
    ATTRIBUTE, CALL, COMMENT, FALSE, FUNCTION_FIELD, IDENTIFIER, LEFT_FIELD, RIGHT_FIELD, TRUE, TUPLE,
    is_string, named, string_prefix, text, unparenthesized,
};
use super::branches::equivalent;
use super::loop_variable_reads::is_read_in_scope;
use crate::rules::PythonRule;

const EXCEPT_CLAUSES: [&str; 2] = ["except_clause", "except_group_clause"];
const RAISE: &str = "raise_statement";
const ASSERT: &str = "assert_statement";
const DICTIONARY: &str = "dictionary";
const DICTIONARY_SPLAT: &str = "dictionary_splat";
const NOT_OPERATOR: &str = "not_operator";
const CONDITIONAL: &str = "conditional_expression";
const FOR: &str = "for_statement";
const IF: &str = "if_statement";
const AS_PATTERN: &str = "as_pattern";
const LIST_SPLAT: &str = "list_splat";
const VALUE_FIELD: &str = "value";
const ARGUMENT_FIELD: &str = "argument";
const ARGUMENTS_FIELD: &str = "arguments";
const ATTRIBUTE_FIELD: &str = "attribute";
const CONDITION_FIELD: &str = "condition";
const CONSEQUENCE_FIELD: &str = "consequence";
const ALTERNATIVE_FIELD: &str = "alternative";
const TYPE_FIELD: &str = "type";
const OPERATORS_FIELD: &str = "operators";
const EXCEPTION_KINDS: [&str; 5] = [IDENTIFIER, ATTRIBUTE, "subscript", CALL, "binary_operator"];
const LITERAL_KINDS: [&str; 7] = [
    "integer",
    "float",
    TRUE,
    FALSE,
    "none",
    "ellipsis",
    "concatenated_string",
];
const FORMATTED_PREFIXES: [char; 4] = ['f', 'F', 't', 'T'];
const PAIR_TARGETS: [&str; 3] = ["pattern_list", "tuple_pattern", TUPLE];
const ITEMS: &str = "items";
const ORDERINGS: [&str; 4] = ["<", "<=", ">", ">="];
const ASSIGNMENT: &str = "assignment";
const EXPRESSION_STATEMENT: &str = "expression_statement";

// Chequeos sobre literales y sentencias simples: B030, B016, PLW0129, PIE800, SIM208, SIM211, RUF034,
// PERF102 y PLR1730.
pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    let mut found = Vec::new();
    for node in nodes {
        match node.kind() {
            kind if EXCEPT_CLAUSES.contains(&kind) => found.extend(non_exception_classes(*node)),
            RAISE => found.extend(raised_literal(*node, source)),
            ASSERT => found.extend(asserted_string(*node)),
            DICTIONARY => found.extend(unnecessary_spreads(*node)),
            NOT_OPERATOR => found.extend(double_negation(*node)),
            CONDITIONAL => found.extend(conditional_reports(*node, source)),
            FOR => found.extend(incorrect_dict_iterator(*node, source)),
            IF => found.extend(min_max_if(*node, source)),
            _ => {}
        }
    }
    found
}

fn significant(node: Node<'_>) -> Vec<Node<'_>> {
    named(node)
        .into_iter()
        .filter(|child| child.kind() != COMMENT)
        .collect()
}

// B030: un `except` con algo que no puede ser una clase de excepción (un literal, por ejemplo).
fn non_exception_classes(handler: Node<'_>) -> Vec<Report> {
    let Some(value) = handler.child_by_field_name(VALUE_FIELD) else {
        return Vec::new();
    };
    let caught = if value.kind() == AS_PATTERN {
        significant(value).first().copied().unwrap_or(value)
    } else {
        value
    };
    let caught = unparenthesized(caught);
    let elements = if caught.kind() == TUPLE {
        significant(caught)
    } else {
        vec![caught]
    };
    elements
        .into_iter()
        .map(unparenthesized)
        .filter(|element| !EXCEPTION_KINDS.contains(&element.kind()) && element.kind() != LIST_SPLAT)
        .map(|element| Report::at(PythonRule::B030, element, ""))
        .collect()
}

fn is_plain_literal(node: Node<'_>, source: &[u8]) -> bool {
    LITERAL_KINDS.contains(&node.kind())
        || (is_string(node) && !string_prefix(node, source).contains(FORMATTED_PREFIXES))
}

// B016: `raise` de un literal.
fn raised_literal(statement: Node<'_>, source: &[u8]) -> Option<Report> {
    let exception = unparenthesized(*significant(statement).first()?);
    is_plain_literal(exception, source).then(|| Report::at(PythonRule::B016, exception, ""))
}

// PLW0129: `assert` de un string, que siempre pasa (o nunca, si está vacío).
fn asserted_string(statement: Node<'_>) -> Option<Report> {
    let test = unparenthesized(*significant(statement).first()?);
    is_string(test).then(|| Report::at(PythonRule::PLW0129, test, ""))
}

// PIE800: `{**base, **{"a": 1}}`, donde el diccionario de adentro se puede escribir directo.
fn unnecessary_spreads(dictionary: Node<'_>) -> Vec<Report> {
    significant(dictionary)
        .into_iter()
        .filter(|item| item.kind() == DICTIONARY_SPLAT)
        .filter_map(|splat| significant(splat).first().map(|inner| unparenthesized(*inner)))
        .filter(|inner| inner.kind() == DICTIONARY)
        .map(|inner| Report::at(PythonRule::PIE800, inner, ""))
        .collect()
}

// SIM208: `not not x`.
fn double_negation(operation: Node<'_>) -> Option<Report> {
    let operand = unparenthesized(operation.child_by_field_name(ARGUMENT_FIELD)?);
    (operand.kind() == NOT_OPERATOR).then(|| Report::at(PythonRule::SIM208, operation, ""))
}

// SIM211 (`False if x else True`) y RUF034 (las dos ramas iguales).
fn conditional_reports(conditional: Node<'_>, source: &[u8]) -> Vec<Report> {
    let parts = significant(conditional);
    let [body, _, orelse] = parts.as_slice() else {
        return Vec::new();
    };
    let (body, orelse) = (unparenthesized(*body), unparenthesized(*orelse));
    let mut found = Vec::new();
    if body.kind() == FALSE && orelse.kind() == TRUE {
        found.push(Report::at(PythonRule::SIM211, conditional, ""));
    }
    if equivalent(body, orelse, source) {
        found.push(Report::at(PythonRule::RUF034, conditional, ""));
    }
    found
}

// PERF102: `for key, value in d.items()` que descarta la clave o el valor. Como `is_unused` de
// ruff, un nombre cuenta como usado si alguna variable con ese nombre se lee en el mismo alcance.
fn incorrect_dict_iterator(statement: Node<'_>, source: &[u8]) -> Option<Report> {
    let target = statement.child_by_field_name(LEFT_FIELD)?;
    if !PAIR_TARGETS.contains(&target.kind()) {
        return None;
    }
    let elements = significant(target);
    let [key, value] = elements.as_slice() else {
        return None;
    };
    let iterable = unparenthesized(statement.child_by_field_name(RIGHT_FIELD)?);
    let function = iterable
        .child_by_field_name(FUNCTION_FIELD)
        .filter(|_| iterable.kind() == CALL)?;
    let no_arguments = iterable
        .child_by_field_name(ARGUMENTS_FIELD)
        .is_some_and(|arguments| significant(arguments).is_empty());
    let calls_items = function.kind() == ATTRIBUTE
        && function
            .child_by_field_name(ATTRIBUTE_FIELD)
            .is_some_and(|attribute| text(attribute, source) == ITEMS);
    if !no_arguments || !calls_items {
        return None;
    }
    let key_used = is_any_read(*key, source);
    let value_used = is_any_read(*value, source);
    (key_used != value_used).then(|| Report::at(PythonRule::PERF102, iterable, ""))
}

// `is_unused` de ruff para una tupla: se usa si alguno de sus nombres se usa.
fn is_any_read(target: Node<'_>, source: &[u8]) -> bool {
    if target.kind() == IDENTIFIER {
        return is_read_in_scope(target, source);
    }
    significant(target)
        .into_iter()
        .any(|inner| is_any_read(inner, source))
}

// PLR1730: `if a > b: b = a`, que es `b = max(b, a)`.
fn min_max_if(statement: Node<'_>, source: &[u8]) -> Option<Report> {
    if statement.child_by_field_name(ALTERNATIVE_FIELD).is_some() {
        return None;
    }
    let condition = unparenthesized(statement.child_by_field_name(CONDITION_FIELD)?);
    let operands = significant(condition);
    let [left, right] = operands.as_slice() else {
        return None;
    };
    let mut cursor = condition.walk();
    let operators: Vec<&str> = condition
        .children_by_field_name(OPERATORS_FIELD, &mut cursor)
        .map(|operator| text(operator, source))
        .collect();
    if !matches!(operators.as_slice(), [operator] if ORDERINGS.contains(operator)) {
        return None;
    }
    let body = significant(statement.child_by_field_name(CONSEQUENCE_FIELD)?);
    let [only] = body.as_slice() else {
        return None;
    };
    let assignment = significant(*only)
        .first()
        .copied()
        .filter(|_| only.kind() == EXPRESSION_STATEMENT)
        .filter(|assignment| {
            assignment.kind() == ASSIGNMENT && assignment.child_by_field_name(TYPE_FIELD).is_none()
        })?;
    let target = assignment.child_by_field_name(LEFT_FIELD)?;
    let value = assignment.child_by_field_name(RIGHT_FIELD)?;
    if value.kind() == ASSIGNMENT {
        return None;
    }
    let same = |first: Node<'_>, second: Node<'_>| equivalent(first, second, source);
    let selects =
        (same(target, *left) && same(value, *right)) || (same(target, *right) && same(value, *left));
    selects.then(|| Report::at(PythonRule::PLR1730, statement, ""))
}
