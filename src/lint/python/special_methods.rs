use tree_sitter::Node;

use super::Report;
use super::ast::{
    CALL, CLASS, COMMENT, DECORATED, FALSE, FUNCTION, LAMBDA, NAME_FIELD, TRUE, is_bytes, is_string, named,
    text, unparenthesized,
};
use super::methods::method_kind;
use crate::rules::PythonRule;

const RETURN: &str = "return_statement";
const RAISE: &str = "raise_statement";
const PASS: &str = "pass_statement";
const EXPRESSION_STATEMENT: &str = "expression_statement";
const ELLIPSIS: &str = "ellipsis";
const BODY_FIELD: &str = "body";
const FUNCTION_FIELD: &str = "function";
const NOT_IMPLEMENTED_NAMES: [&str; 2] = ["NotImplementedError", "NotImplemented"];
const DECORATOR: &str = "decorator";
const PARAMETERS_FIELD: &str = "parameters";
const ARGUMENT_FIELD: &str = "argument";
const OPERATOR_FIELD: &str = "operator";
const INIT: &str = "__init__";
const NONE_LITERAL: &str = "none";
const INTEGER: &str = "integer";
const FLOAT: &str = "float";
const UNARY_OPERATOR: &str = "unary_operator";
const NOT_OPERATOR: &str = "not_operator";
const MINUS: &str = "-";
const IMAGINARY_SUFFIXES: [char; 2] = ['j', 'J'];
const PROPERTY_DECORATORS: [&str; 2] = ["property", "builtins.property"];
const SEPARATORS: [&str; 4] = [
    "positional_separator",
    "keyword_separator",
    "list_splat_pattern",
    "dictionary_splat_pattern",
];
const SPLATS: [&str; 2] = ["list_splat_pattern", "dictionary_splat_pattern"];
const COLLECTIONS: [&str; 8] = [
    "list",
    "tuple",
    "dictionary",
    "set",
    "list_comprehension",
    "set_comprehension",
    "dictionary_comprehension",
    "generator_expression",
];

// `ResolvedPythonType` de ruff para lo que devuelve un método especial; `None` si no se sabe.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Resolved {
    Integer { negative: bool },
    Float,
    Complex,
    Bool,
    Text,
    Bytes,
    NoneValue,
    Collection,
}

// PLE0101 (`__init__` que devuelve algo), PLE0303 a PLE0309 (`__len__`, `__bool__`, `__index__`,
// `__str__`, `__bytes__` y `__hash__` que devuelven un tipo equivocado) y PLR0206 (una propiedad con
// parámetros).
pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    nodes
        .iter()
        .filter(|node| node.kind() == FUNCTION && method_kind(**node, source).is_some())
        .flat_map(|function| method_reports(*function, source))
        .collect()
}

fn method_reports(function: Node<'_>, source: &[u8]) -> Vec<Report> {
    let Some(name) = function.child_by_field_name(NAME_FIELD) else {
        return Vec::new();
    };
    let mut found: Vec<Report> = property_with_parameters(function, name, source)
        .into_iter()
        .collect();
    let name = text(name, source);
    if name == INIT {
        found.extend(
            returns(function)
                .into_iter()
                .filter(|statement| returned(*statement).is_some_and(|value| value.kind() != NONE_LITERAL))
                .map(|statement| Report::at(PythonRule::PLE0101, statement, "")),
        );
        return found;
    }
    let Some((rule, allowed)) = expected_return(name) else {
        return found;
    };
    let statements = returns(function);
    if statements.is_empty() && !is_stub(function, source) && !ends_raising(function) {
        found.push(Report::at(
            rule,
            function.child_by_field_name(NAME_FIELD).unwrap_or(function),
            "",
        ));
    }
    for statement in statements {
        match returned(statement) {
            None => found.push(Report::at(rule, statement, "")),
            Some(value) => {
                if resolved(value, source).is_some_and(|kind| !allowed(kind)) {
                    found.push(Report::at(rule, value, ""));
                }
            }
        }
    }
    found
}

type Allowed = fn(Resolved) -> bool;

fn expected_return(name: &str) -> Option<(PythonRule, Allowed)> {
    let rule: (PythonRule, Allowed) = match name {
        "__len__" => (PythonRule::PLE0303, |kind| {
            kind == Resolved::Integer { negative: false }
        }),
        "__bool__" => (PythonRule::PLE0304, |kind| kind == Resolved::Bool),
        "__index__" => (PythonRule::PLE0305, |kind| {
            matches!(kind, Resolved::Integer { .. })
        }),
        "__str__" => (PythonRule::PLE0307, |kind| kind == Resolved::Text),
        "__bytes__" => (PythonRule::PLE0308, |kind| kind == Resolved::Bytes),
        "__hash__" => (PythonRule::PLE0309, |kind| {
            matches!(kind, Resolved::Integer { .. })
        }),
        _ => return None,
    };
    Some(rule)
}

fn body(function: Node<'_>) -> Vec<Node<'_>> {
    function
        .child_by_field_name(BODY_FIELD)
        .map(named)
        .unwrap_or_default()
        .into_iter()
        .filter(|statement| statement.kind() != COMMENT)
        .collect()
}

// `is_stub` de ruff: sólo `pass`, `...`, un docstring o `raise NotImplementedError`.
fn is_stub(function: Node<'_>, source: &[u8]) -> bool {
    body(function)
        .into_iter()
        .all(|statement| match statement.kind() {
            PASS => true,
            EXPRESSION_STATEMENT => named(statement)
                .first()
                .is_some_and(|value| value.kind() == ELLIPSIS || is_string(*value)),
            RAISE => named(statement).first().is_some_and(|exception| {
                let exception = if exception.kind() == CALL {
                    exception
                        .child_by_field_name(FUNCTION_FIELD)
                        .unwrap_or(*exception)
                } else {
                    *exception
                };
                NOT_IMPLEMENTED_NAMES.contains(&text(exception, source))
            }),
            _ => false,
        })
}

fn ends_raising(function: Node<'_>) -> bool {
    body(function)
        .last()
        .is_some_and(|statement| statement.kind() == RAISE)
}

fn returned(statement: Node<'_>) -> Option<Node<'_>> {
    named(statement)
        .into_iter()
        .find(|child| child.kind() != COMMENT)
        .map(unparenthesized)
}

// Los `return` de la función, sin entrar en funciones, lambdas ni clases de adentro.
fn returns(function: Node<'_>) -> Vec<Node<'_>> {
    let mut found = Vec::new();
    let mut pending: Vec<Node<'_>> = named(function);
    while let Some(node) = pending.pop() {
        match node.kind() {
            RETURN => found.push(node),
            FUNCTION | LAMBDA | CLASS | DECORATED => {}
            _ => pending.extend(named(node)),
        }
    }
    found
}

fn resolved(value: Node<'_>, source: &[u8]) -> Option<Resolved> {
    match value.kind() {
        INTEGER if text(value, source).ends_with(IMAGINARY_SUFFIXES) => Some(Resolved::Complex),
        INTEGER => Some(Resolved::Integer { negative: false }),
        FLOAT => Some(Resolved::Float),
        TRUE | FALSE | NOT_OPERATOR => Some(Resolved::Bool),
        NONE_LITERAL => Some(Resolved::NoneValue),
        kind if COLLECTIONS.contains(&kind) => Some(Resolved::Collection),
        _ if is_string(value) && is_bytes(value, source) => Some(Resolved::Bytes),
        _ if is_string(value) => Some(Resolved::Text),
        UNARY_OPERATOR => {
            let operand = unparenthesized(value.child_by_field_name(ARGUMENT_FIELD)?);
            let negative = value
                .child_by_field_name(OPERATOR_FIELD)
                .is_some_and(|operator| text(operator, source) == MINUS);
            match resolved(operand, source)? {
                Resolved::Integer { .. } | Resolved::Bool => Some(Resolved::Integer { negative }),
                number @ (Resolved::Float | Resolved::Complex) => Some(number),
                Resolved::Text | Resolved::Bytes | Resolved::NoneValue | Resolved::Collection => None,
            }
        }
        _ => None,
    }
}

// PLR0206: `@property` en un método que recibe algo más que `self`. Avisa en el nombre.
fn property_with_parameters(function: Node<'_>, name: Node<'_>, source: &[u8]) -> Option<Report> {
    let holder = function.parent().filter(|parent| parent.kind() == DECORATED)?;
    let is_property = named(holder)
        .into_iter()
        .filter(|child| child.kind() == DECORATOR)
        .filter_map(|decorator| named(decorator).into_iter().find(|child| child.kind() != COMMENT))
        .any(|expression| PROPERTY_DECORATORS.contains(&text(expression, source)));
    let parameters = named(function.child_by_field_name(PARAMETERS_FIELD)?)
        .into_iter()
        .filter(|parameter| parameter.kind() != COMMENT && !SEPARATORS.contains(&parameter.kind()))
        .filter(|parameter| {
            !named(*parameter)
                .iter()
                .any(|inner| SPLATS.contains(&inner.kind()))
        })
        .count();
    (is_property && parameters > 1).then(|| Report::at(PythonRule::PLR0206, name, ""))
}
