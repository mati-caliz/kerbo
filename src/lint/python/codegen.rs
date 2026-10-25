use tree_sitter::Node;

use super::ast::{COMMENT, named, text};
use containers::{comprehension, dictionary, generic_type, items, member_type, slice, tuple, union_type};

mod containers;

// Las precedencias del generador de ruff (`ruff_python_codegen`).
pub const NAMED_EXPR: u8 = 1;
pub const YIELD: u8 = 7;
pub const SUBSCRIPT: u8 = 13;
pub const COMPREHENSION_TARGET: u8 = 19;
pub const TUPLE: u8 = 19;
pub const COMMA: u8 = 21;
pub const COMPREHENSION_ELEMENT: u8 = 27;
pub const LAMBDA: u8 = 27;
pub const IF_EXP: u8 = 27;
pub const COMPREHENSION: u8 = 29;
pub const OR: u8 = 31;
pub const AND: u8 = 33;
pub const NOT: u8 = 35;
pub const CMP: u8 = 37;
pub const BIT_OR: u8 = 39;
pub const UNARY: u8 = 53;
pub const POW: u8 = 55;
pub const AWAIT: u8 = 57;
pub const MAX: u8 = 63;

const BINARY_PRECEDENCES: [(&str, u8); 13] = [
    ("|", BIT_OR),
    ("^", 41),
    ("&", 43),
    ("<<", 45),
    (">>", 45),
    ("+", 47),
    ("-", 47),
    ("*", 49),
    ("/", 49),
    ("%", 49),
    ("//", 49),
    ("@", 49),
    ("**", POW),
];
const POWER: &str = "**";
const AND_KEYWORD: &str = "and";
const OPERATOR_FIELD: &str = "operator";
const OPERATORS_FIELD: &str = "operators";
const LEFT_FIELD: &str = "left";
const RIGHT_FIELD: &str = "right";
const ARGUMENT_FIELD: &str = "argument";
const BODY_FIELD: &str = "body";
const NAME_FIELD: &str = "name";
const VALUE_FIELD: &str = "value";
const KEY_FIELD: &str = "key";
const OBJECT_FIELD: &str = "object";
const ATTRIBUTE_FIELD: &str = "attribute";
const FUNCTION_FIELD: &str = "function";
const ARGUMENTS_FIELD: &str = "arguments";
const SUBSCRIPT_FIELD: &str = "subscript";
const PARAMETERS_FIELD: &str = "parameters";
const INTEGER: &str = "integer";
const GENERATOR: &str = "generator_expression";
const ASYNC: &str = "async";
const AWAIT_KIND: &str = "await";
const BINARY_OPERATOR: &str = "binary_operator";
const BOOLEAN_OPERATOR: &str = "boolean_operator";
const CALL: &str = "call";
const COMPARISON_OPERATOR: &str = "comparison_operator";
const CONCATENATED_STRING: &str = "concatenated_string";
const CONDITIONAL_EXPRESSION: &str = "conditional_expression";
const DICTIONARY: &str = "dictionary";
const DICTIONARY_COMPREHENSION: &str = "dictionary_comprehension";
const DICTIONARY_SPLAT: &str = "dictionary_splat";
const EXPRESSION_LIST: &str = "expression_list";
const FOR_IN_CLAUSE: &str = "for_in_clause";
const GENERIC_TYPE: &str = "generic_type";
const IF_CLAUSE: &str = "if_clause";
const KEYWORD_ARGUMENT: &str = "keyword_argument";
const LAMBDA_KIND: &str = "lambda";
const LIST: &str = "list";
const LIST_COMPREHENSION: &str = "list_comprehension";
const LIST_SPLAT: &str = "list_splat";
const LIST_SPLAT_PATTERN: &str = "list_splat_pattern";
const MEMBER_TYPE: &str = "member_type";
const NAMED_EXPRESSION: &str = "named_expression";
const NOT_OPERATOR: &str = "not_operator";
const PAIR: &str = "pair";
const PARENTHESIZED_EXPRESSION: &str = "parenthesized_expression";
const SET: &str = "set";
const SET_COMPREHENSION: &str = "set_comprehension";
const SLICE: &str = "slice";
const TUPLE_KIND: &str = "tuple";
const TYPE: &str = "type";
const UNARY_OPERATOR: &str = "unary_operator";
const UNION_TYPE: &str = "union_type";
const YIELD_KIND: &str = "yield";

// El `unparse_expr` del generador de ruff: vuelve a imprimir la expresión con los espacios normales y
// los paréntesis que pide la precedencia de `level`. Los strings y los números quedan como en el código.
pub fn unparse(node: Node<'_>, level: u8, source: &[u8]) -> String {
    match node.kind() {
        PARENTHESIZED_EXPRESSION => significant(node).first().map_or_else(
            || text(node, source).to_owned(),
            |inner| unparse(*inner, level, source),
        ),
        BOOLEAN_OPERATOR => boolean(node, level, source),
        NOT_OPERATOR => node
            .child_by_field_name(ARGUMENT_FIELD)
            .map_or_else(String::new, |operand| {
                group(level, NOT, format!("not {}", unparse(operand, NOT, source)))
            }),
        BINARY_OPERATOR => binary(node, level, source),
        UNARY_OPERATOR => unary(node, level, source),
        COMPARISON_OPERATOR => comparison(node, level, source),
        CONDITIONAL_EXPRESSION => conditional(node, level, source),
        NAMED_EXPRESSION => named_expression(node, level, source),
        LAMBDA_KIND => lambda(node, level, source),
        AWAIT_KIND => group(level, AWAIT, format!("await {}", first(node, MAX, source))),
        YIELD_KIND => yield_expression(node, level, source),
        ATTRIBUTE_FIELD => attribute(node, source),
        SUBSCRIPT_FIELD => subscript(node, source),
        CALL => call(node, source),
        LIST => format!("[{}]", items(node, source)),
        SET => format!("{{{}}}", items(node, source)),
        TUPLE_KIND | EXPRESSION_LIST => tuple(node, level, source),
        DICTIONARY => dictionary(node, source),
        LIST_SPLAT | LIST_SPLAT_PATTERN => format!("*{}", first(node, MAX, source)),
        DICTIONARY_SPLAT => format!("**{}", first(node, MAX, source)),
        LIST_COMPREHENSION => format!("[{}]", comprehension(node, source)),
        SET_COMPREHENSION | DICTIONARY_COMPREHENSION => format!("{{{}}}", comprehension(node, source)),
        GENERATOR => format!("({})", comprehension(node, source)),
        CONCATENATED_STRING => significant(node)
            .iter()
            .map(|part| text(*part, source))
            .collect::<Vec<_>>()
            .join(" "),
        SLICE => slice(node, source),
        TYPE => first(node, level, source),
        UNION_TYPE => union_type(node, level, source),
        GENERIC_TYPE => generic_type(node, source),
        MEMBER_TYPE => member_type(node, source),
        _ => text(node, source).to_owned(),
    }
}

pub fn group(level: u8, precedence: u8, body: String) -> String {
    if level > precedence {
        format!("({body})")
    } else {
        body
    }
}

fn significant(node: Node<'_>) -> Vec<Node<'_>> {
    named(node)
        .into_iter()
        .filter(|child| child.kind() != COMMENT)
        .collect()
}

fn first(node: Node<'_>, level: u8, source: &[u8]) -> String {
    significant(node)
        .first()
        .map_or_else(String::new, |inner| unparse(*inner, level, source))
}

fn operator_text(node: Node<'_>, source: &[u8]) -> String {
    node.child_by_field_name(OPERATOR_FIELD)
        .map(|operator| text(operator, source).to_owned())
        .unwrap_or_default()
}

// `a and b and c` es un solo `BoolOp` en ruff: los operandos de la cadena van todos un nivel arriba.
fn boolean(node: Node<'_>, level: u8, source: &[u8]) -> String {
    let operator = operator_text(node, source);
    let precedence = if operator == AND_KEYWORD { AND } else { OR };
    let mut operands = Vec::new();
    collect_boolean(node, &operator, source, &mut operands);
    let body = operands
        .iter()
        .map(|operand| unparse(*operand, precedence.saturating_add(1), source))
        .collect::<Vec<_>>()
        .join(&format!(" {operator} "));
    group(level, precedence, body)
}

fn collect_boolean<'tree>(node: Node<'tree>, operator: &str, source: &[u8], operands: &mut Vec<Node<'tree>>) {
    for side in [LEFT_FIELD, RIGHT_FIELD]
        .into_iter()
        .filter_map(|field| node.child_by_field_name(field))
    {
        if side.kind() == BOOLEAN_OPERATOR && operator_text(side, source) == operator {
            collect_boolean(side, operator, source, operands);
        } else {
            operands.push(side);
        }
    }
}

fn binary(node: Node<'_>, level: u8, source: &[u8]) -> String {
    let operator = operator_text(node, source);
    let precedence = BINARY_PRECEDENCES
        .iter()
        .find(|(symbol, _)| *symbol == operator)
        .map_or(MAX, |(_, precedence)| *precedence);
    let right_associative = u8::from(operator == POWER);
    let left = node
        .child_by_field_name(LEFT_FIELD)
        .map(|left| unparse(left, precedence.saturating_add(right_associative), source))
        .unwrap_or_default();
    let right = node
        .child_by_field_name(RIGHT_FIELD)
        .map(|right| unparse(right, precedence.saturating_add(1 - right_associative), source))
        .unwrap_or_default();
    group(level, precedence, format!("{left} {operator} {right}"))
}

fn unary(node: Node<'_>, level: u8, source: &[u8]) -> String {
    let operator = operator_text(node, source);
    let operand = node
        .child_by_field_name(ARGUMENT_FIELD)
        .map(|operand| unparse(operand, UNARY, source))
        .unwrap_or_default();
    group(level, UNARY, format!("{operator}{operand}"))
}

fn comparison(node: Node<'_>, level: u8, source: &[u8]) -> String {
    let operands = significant(node);
    let mut cursor = node.walk();
    let operators: Vec<String> = node
        .children_by_field_name(OPERATORS_FIELD, &mut cursor)
        .map(|operator| {
            text(operator, source)
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    let mut body = operands
        .first()
        .map(|left| unparse(*left, CMP.saturating_add(1), source))
        .unwrap_or_default();
    for (operator, operand) in operators.iter().zip(operands.iter().skip(1)) {
        body.push(' ');
        body.push_str(operator);
        body.push(' ');
        body.push_str(&unparse(*operand, CMP.saturating_add(1), source));
    }
    group(level, CMP, body)
}

fn conditional(node: Node<'_>, level: u8, source: &[u8]) -> String {
    let parts = significant(node);
    let [body, test, orelse] = parts.as_slice() else {
        return text(node, source).to_owned();
    };
    let inner = IF_EXP.saturating_add(1);
    group(
        level,
        IF_EXP,
        format!(
            "{} if {} else {}",
            unparse(*body, inner, source),
            unparse(*test, inner, source),
            unparse(*orelse, IF_EXP, source)
        ),
    )
}

fn named_expression(node: Node<'_>, level: u8, source: &[u8]) -> String {
    let name = node
        .child_by_field_name(NAME_FIELD)
        .map(|name| text(name, source).to_owned())
        .unwrap_or_default();
    let value = node
        .child_by_field_name(VALUE_FIELD)
        .map(|value| unparse(value, NAMED_EXPR.saturating_add(1), source))
        .unwrap_or_default();
    group(level, NAMED_EXPR, format!("{name} := {value}"))
}

fn lambda(node: Node<'_>, level: u8, source: &[u8]) -> String {
    let parameters = node
        .child_by_field_name(PARAMETERS_FIELD)
        .map(|parameters| format!(" {}", text(parameters, source)))
        .unwrap_or_default();
    let body = node
        .child_by_field_name(BODY_FIELD)
        .map(|body| unparse(body, LAMBDA, source))
        .unwrap_or_default();
    group(level, LAMBDA, format!("lambda{parameters}: {body}"))
}

fn yield_expression(node: Node<'_>, level: u8, source: &[u8]) -> String {
    let value = significant(node)
        .first()
        .map(|value| format!(" {}", unparse(*value, YIELD.saturating_add(1), source)))
        .unwrap_or_default();
    group(level, YIELD, format!("yield{value}"))
}

fn attribute(node: Node<'_>, source: &[u8]) -> String {
    let object = node.child_by_field_name(OBJECT_FIELD);
    let name = node
        .child_by_field_name(ATTRIBUTE_FIELD)
        .map(|name| text(name, source))
        .unwrap_or_default();
    match object {
        Some(object) if object.kind() == INTEGER => format!("({}).{name}", text(object, source)),
        Some(object) => format!("{}.{name}", unparse(object, MAX, source)),
        None => text(node, source).to_owned(),
    }
}

fn subscript(node: Node<'_>, source: &[u8]) -> String {
    let value = node
        .child_by_field_name(VALUE_FIELD)
        .map(|value| unparse(value, MAX, source))
        .unwrap_or_default();
    let mut cursor = node.walk();
    let slices: Vec<Node<'_>> = node
        .children_by_field_name(SUBSCRIPT_FIELD, &mut cursor)
        .filter(|slice| slice.kind() != COMMENT)
        .collect();
    let inner = match slices.as_slice() {
        [only] => unparse(*only, SUBSCRIPT, source),
        _ => slices
            .iter()
            .map(|slice| unparse(*slice, COMMA, source))
            .collect::<Vec<_>>()
            .join(", "),
    };
    format!("{value}[{inner}]")
}

fn call(node: Node<'_>, source: &[u8]) -> String {
    let function = node
        .child_by_field_name(FUNCTION_FIELD)
        .map(|function| unparse(function, MAX, source))
        .unwrap_or_default();
    let Some(arguments) = node.child_by_field_name(ARGUMENTS_FIELD) else {
        return function;
    };
    if arguments.kind() == GENERATOR {
        return format!("{function}({})", comprehension(arguments, source));
    }
    let rendered: Vec<String> = significant(arguments)
        .into_iter()
        .map(|argument| match argument.kind() {
            KEYWORD_ARGUMENT => {
                let name = argument
                    .child_by_field_name(NAME_FIELD)
                    .map(|name| text(name, source))
                    .unwrap_or_default();
                let value = argument
                    .child_by_field_name(VALUE_FIELD)
                    .map(|value| unparse(value, COMMA, source))
                    .unwrap_or_default();
                format!("{name}={value}")
            }
            _ => unparse(argument, COMMA, source),
        })
        .collect();
    format!("{function}({})", rendered.join(", "))
}

#[cfg(test)]
mod tests;
