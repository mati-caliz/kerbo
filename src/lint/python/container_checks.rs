use tree_sitter::Node;

use super::Report;
use super::ast::{
    ARGUMENTS_FIELD, ASSIGNMENT, ATTRIBUTE, ATTRIBUTE_FIELD, CALL, COMMENT, EXPRESSION_STATEMENT,
    FUNCTION_FIELD, IDENTIFIER, KEYWORD_ARGUMENT, LEFT_FIELD, LIST, OBJECT_FIELD, RIGHT_FIELD, SUBSCRIPT,
    SUBSCRIPT_FIELD, TUPLE, TYPE_FIELD, VALUE_FIELD, is_string, named, text, unparenthesized,
};
use super::branches::equivalent;
use super::escapes::text_value;
use super::statement_removal::delete_statement;
use crate::rules::PythonRule;

const IF: &str = "if_statement";
const CONDITIONAL: &str = "conditional_expression";
const SLICE: &str = "slice";
const UNARY: &str = "unary_operator";
const INTEGER: &str = "integer";
const PASS: &str = "pass_statement";
const CONDITION_FIELD: &str = "condition";
const CONSEQUENCE_FIELD: &str = "consequence";
const ALTERNATIVE_FIELD: &str = "alternative";
const OPERATOR_FIELD: &str = "operator";
const ARGUMENT_FIELD: &str = "argument";
const COLON: &str = ":";
const MINUS: &str = "-";
const STARTSWITH: &str = "startswith";
const ENDSWITH: &str = "endswith";
const LEN: &str = "len";
const ONE: &str = "1";
const ALWAYS_UNIT_STEPS: [&str; 2] = ["none", "true"];
const DUNDER_DICT: &str = "__dict__";
const ANNOTATIONS: &str = "__annotations__";
const GET: &str = "get";
const MAX_GET_ARGUMENTS: usize = 2;
const TYPE_CHECKING: &str = "TYPE_CHECKING";
const QUALIFIED_TYPE_CHECKING: &str = ".TYPE_CHECKING";
const HEX_PREFIXES: [&str; 2] = ["0x", "0X"];
const OCTAL_PREFIXES: [&str; 2] = ["0o", "0O"];
const BINARY_PREFIXES: [&str; 2] = ["0b", "0B"];
const DIGIT_SEPARATOR: char = '_';
const RADIX_PREFIXES: [([&str; 2], u32); 3] = [(HEX_PREFIXES, 16), (OCTAL_PREFIXES, 8), (BINARY_PREFIXES, 2)];

// FURB188 (`s[3:] if s.startswith("pre") else s`), PLE0643 (`[0, 1][2]`), RUF063
// (`cls.__dict__["__annotations__"]`) y TC005 (`if TYPE_CHECKING: pass`).
pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    let mut found = Vec::new();
    for node in nodes {
        match node.kind() {
            CONDITIONAL => found.extend(affix_expression(*node, source)),
            IF => {
                found.extend(affix_statement(*node, source));
                found.extend(empty_type_checking(*node, source));
            }
            SUBSCRIPT => {
                found.extend(index_error(*node, source));
                found.extend(annotations_by_key(*node, source));
            }
            CALL => found.extend(annotations_with_get(*node, source)),
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

// FURB188 en su forma de expresión: `texto[n:] if texto.startswith(p) else texto`.
fn affix_expression(conditional: Node<'_>, source: &[u8]) -> Option<Report> {
    let parts = significant(conditional);
    let [body, test, orelse] = parts.as_slice() else {
        return None;
    };
    let body = unparenthesized(*body);
    (body.kind() == SUBSCRIPT && removes_affix(body, *test, *orelse, source))
        .then(|| Report::at(PythonRule::FURB188, conditional, ""))
}

// FURB188 como sentencia: `if texto.startswith(p): texto = texto[n:]`, sin `else`.
fn affix_statement(statement: Node<'_>, source: &[u8]) -> Option<Report> {
    if statement.child_by_field_name(ALTERNATIVE_FIELD).is_some() {
        return None;
    }
    let test = statement.child_by_field_name(CONDITION_FIELD)?;
    let body = significant(statement.child_by_field_name(CONSEQUENCE_FIELD)?);
    let [only] = body.as_slice() else {
        return None;
    };
    let assignment = significant(*only)
        .first()
        .copied()
        .filter(|assignment| only.kind() == EXPRESSION_STATEMENT && assignment.kind() == ASSIGNMENT)?;
    if assignment.child_by_field_name(TYPE_FIELD).is_some() {
        return None;
    }
    let target = assignment.child_by_field_name(LEFT_FIELD)?;
    let value = assignment
        .child_by_field_name(RIGHT_FIELD)
        .map(unparenthesized)
        .filter(|value| value.kind() == SUBSCRIPT)?;
    removes_affix(value, test, target, source).then(|| Report::at(PythonRule::FURB188, statement, ""))
}

struct SliceBounds<'tree> {
    lower: Option<Node<'tree>>,
    upper: Option<Node<'tree>>,
    step: Option<Node<'tree>>,
}

fn slice_bounds(slice: Node<'_>) -> SliceBounds<'_> {
    let mut sections: [Option<Node<'_>>; 3] = [None, None, None];
    let mut position = 0;
    let mut cursor = slice.walk();
    for child in slice.children(&mut cursor) {
        if child.kind() == COLON {
            position += 1;
        } else if child.is_named()
            && child.kind() != COMMENT
            && let Some(section) = sections.get_mut(position)
        {
            *section = Some(child);
        }
    }
    let [lower, upper, step] = sections;
    SliceBounds { lower, upper, step }
}

fn removes_affix(subscript: Node<'_>, test: Node<'_>, kept: Node<'_>, source: &[u8]) -> bool {
    let (Some(value), Some(slice)) = (
        subscript.child_by_field_name(VALUE_FIELD),
        subscript.child_by_field_name(SUBSCRIPT_FIELD),
    ) else {
        return false;
    };
    if slice.kind() != SLICE || !equivalent(value, kept, source) {
        return false;
    }
    let bounds = slice_bounds(slice);
    let unit_step = bounds.step.is_none_or(|step| {
        let step = unparenthesized(step);
        (step.kind() == INTEGER && text(step, source) == ONE) || ALWAYS_UNIT_STEPS.contains(&step.kind())
    });
    let test = unparenthesized(test);
    if !unit_step || test.kind() != CALL {
        return false;
    }
    let Some(function) = test
        .child_by_field_name(FUNCTION_FIELD)
        .filter(|function| function.kind() == ATTRIBUTE)
    else {
        return false;
    };
    let tested = function.child_by_field_name(OBJECT_FIELD);
    let method = function
        .child_by_field_name(ATTRIBUTE_FIELD)
        .map(|attribute| text(attribute, source));
    let positional: Vec<Node<'_>> = test
        .child_by_field_name(ARGUMENTS_FIELD)
        .map(significant)
        .unwrap_or_default()
        .into_iter()
        .filter(|argument| argument.kind() != KEYWORD_ARGUMENT)
        .collect();
    let ([affix], Some(tested)) = (positional.as_slice(), tested) else {
        return false;
    };
    if !equivalent(tested, value, source) {
        return false;
    }
    match (method, bounds.lower, bounds.upper) {
        (Some(STARTSWITH), Some(lower), None) => matches_length(lower, *affix, false, source),
        (Some(ENDSWITH), None, Some(upper)) => matches_length(upper, *affix, true, source),
        _ => false,
    }
}

// El corte tiene el largo del prefijo: un número igual a los caracteres del literal o `len(prefijo)`;
// para un sufijo, los mismos con un menos adelante.
fn matches_length(bound: Node<'_>, affix: Node<'_>, negated: bool, source: &[u8]) -> bool {
    let mut bound = unparenthesized(bound);
    if negated {
        let is_negation = bound.kind() == UNARY
            && bound
                .child_by_field_name(OPERATOR_FIELD)
                .is_some_and(|operator| text(operator, source) == MINUS);
        let Some(operand) = bound.child_by_field_name(ARGUMENT_FIELD).filter(|_| is_negation) else {
            return false;
        };
        bound = unparenthesized(operand);
    }
    match bound.kind() {
        INTEGER => {
            let affix = unparenthesized(affix);
            is_string(affix)
                && text_value(affix, source)
                    .zip(integer_value(bound, source))
                    .is_some_and(|(literal, length)| i128::try_from(literal.chars().count()) == Ok(length))
        }
        CALL => {
            let is_len = bound
                .child_by_field_name(FUNCTION_FIELD)
                .is_some_and(|function| function.kind() == IDENTIFIER && text(function, source) == LEN);
            let arguments = bound
                .child_by_field_name(ARGUMENTS_FIELD)
                .map(significant)
                .unwrap_or_default();
            is_len && matches!(arguments.as_slice(), [only] if equivalent(*only, affix, source))
        }
        _ => false,
    }
}

pub fn integer_value(integer: Node<'_>, source: &[u8]) -> Option<i128> {
    let literal: String = text(integer, source)
        .chars()
        .filter(|character| *character != DIGIT_SEPARATOR)
        .collect();
    RADIX_PREFIXES
        .iter()
        .find_map(|(prefixes, radix)| {
            prefixes
                .iter()
                .find_map(|prefix| literal.strip_prefix(prefix))
                .map(|digits| i128::from_str_radix(digits, *radix).ok())
        })
        .unwrap_or_else(|| literal.parse().ok())
}

// PLE0643: un índice fijo fuera de una lista o tupla escrita en el código.
fn index_error(subscript: Node<'_>, source: &[u8]) -> Option<Report> {
    let value = unparenthesized(subscript.child_by_field_name(VALUE_FIELD)?);
    if !matches!(value.kind(), LIST | TUPLE) {
        return None;
    }
    let mut cursor = subscript.walk();
    let slices: Vec<Node<'_>> = subscript
        .children_by_field_name(SUBSCRIPT_FIELD, &mut cursor)
        .collect();
    let [slice] = slices.as_slice() else {
        return None;
    };
    let length = i128::try_from(significant(value).len()).ok()?;
    let index = unparenthesized(*slice);
    let position = match index.kind() {
        INTEGER => integer_value(index, source),
        UNARY => {
            let is_negation = index
                .child_by_field_name(OPERATOR_FIELD)
                .is_some_and(|operator| text(operator, source) == MINUS);
            let operand = index
                .child_by_field_name(ARGUMENT_FIELD)
                .filter(|operand| is_negation && operand.kind() == INTEGER)?;
            integer_value(operand, source).map(|number| -number)
        }
        _ => return None,
    };
    let out_of_bounds = position.is_none_or(|position| position >= length || position < -length);
    out_of_bounds.then(|| Report::at(PythonRule::PLE0643, *slice, ""))
}

fn is_class_dict(node: Node<'_>, source: &[u8]) -> bool {
    let node = unparenthesized(node);
    node.kind() == ATTRIBUTE
        && node
            .child_by_field_name(ATTRIBUTE_FIELD)
            .is_some_and(|attribute| text(attribute, source) == DUNDER_DICT)
}

fn is_annotations_key(node: Node<'_>, source: &[u8]) -> bool {
    let node = unparenthesized(node);
    is_string(node) && text_value(node, source).as_deref() == Some(ANNOTATIONS)
}

// RUF063: `cls.__dict__["__annotations__"]`.
fn annotations_by_key(subscript: Node<'_>, source: &[u8]) -> Option<Report> {
    let mut cursor = subscript.walk();
    let slices: Vec<Node<'_>> = subscript
        .children_by_field_name(SUBSCRIPT_FIELD, &mut cursor)
        .collect();
    let [key] = slices.as_slice() else {
        return None;
    };
    let value = subscript.child_by_field_name(VALUE_FIELD)?;
    (is_annotations_key(*key, source) && is_class_dict(value, source))
        .then(|| Report::at(PythonRule::RUF063, subscript, ""))
}

// RUF063: `cls.__dict__.get("__annotations__")`, con un default o sin él.
fn annotations_with_get(call: Node<'_>, source: &[u8]) -> Option<Report> {
    let function = call
        .child_by_field_name(FUNCTION_FIELD)
        .filter(|function| function.kind() == ATTRIBUTE)?;
    let is_get = function
        .child_by_field_name(ATTRIBUTE_FIELD)
        .is_some_and(|attribute| text(attribute, source) == GET);
    if !is_get || !is_class_dict(function.child_by_field_name(OBJECT_FIELD)?, source) {
        return None;
    }
    let arguments = significant(call.child_by_field_name(ARGUMENTS_FIELD)?);
    if arguments.len() > MAX_GET_ARGUMENTS
        || arguments
            .iter()
            .any(|argument| argument.kind() == KEYWORD_ARGUMENT)
    {
        return None;
    }
    let first = *arguments.first()?;
    is_annotations_key(first, source).then(|| Report::at(PythonRule::RUF063, call, ""))
}

// TC005: `if TYPE_CHECKING: pass`, sin `elif` ni `else`.
fn empty_type_checking(statement: Node<'_>, source: &[u8]) -> Option<Report> {
    if statement.child_by_field_name(ALTERNATIVE_FIELD).is_some() {
        return None;
    }
    let condition = text(statement.child_by_field_name(CONDITION_FIELD)?, source);
    if condition != TYPE_CHECKING && !condition.ends_with(QUALIFIED_TYPE_CHECKING) {
        return None;
    }
    let body = significant(statement.child_by_field_name(CONSEQUENCE_FIELD)?);
    match body.as_slice() {
        [only] if only.kind() == PASS => {
            Some(Report::at(PythonRule::TC005, *only, "").with_fix(Some(delete_statement(statement, source))))
        }
        _ => None,
    }
}
