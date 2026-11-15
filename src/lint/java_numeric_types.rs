use tree_sitter::Node;

use super::java::{IDENTIFIER, NAME_FIELD, text, unparenthesized};
use super::java_receivers::declared_type;
use crate::syntax::named_children;

const DECIMAL_INTEGER: &str = "decimal_integer_literal";
const DECIMAL_FLOAT: &str = "decimal_floating_point_literal";
const INVOCATION: &str = "method_invocation";
const UNARY: &str = "unary_expression";
const FIELD_ACCESS: &str = "field_access";
const OBJECT_FIELD: &str = "object";
const ARGUMENTS_FIELD: &str = "arguments";
const OPERATOR_FIELD: &str = "operator";
const OPERAND_FIELD: &str = "operand";
const MINUS: &str = "-";
const SIZE_METHODS: [&str; 2] = ["size", "length"];
const NUMERIC_TYPES: [&str; 4] = ["int", "long", "float", "double"];
const INT: &str = "int";
const LONG: &str = "long";
const FLOAT: &str = "float";
const DOUBLE: &str = "double";
const LONG_SUFFIXES: [char; 2] = ['L', 'l'];
const FLOAT_SUFFIXES: [char; 2] = ['F', 'f'];

// El tipo numérico de un operando de min/max: el declarado de una variable, el de un literal (con su
// sufijo) o el int de `size()` y `length()`.
pub(super) fn numeric_type<'source>(expression: Node<'_>, source: &'source [u8]) -> Option<&'source str> {
    let expression = unparenthesized(expression);
    match expression.kind() {
        IDENTIFIER => {
            let written = declared_type(expression, text(expression, source), source)?;
            let written = text(written, source);
            NUMERIC_TYPES.iter().find(|numeric| **numeric == written).copied()
        }
        DECIMAL_INTEGER => Some(if text(expression, source).ends_with(LONG_SUFFIXES) {
            LONG
        } else {
            INT
        }),
        DECIMAL_FLOAT => Some(if text(expression, source).ends_with(FLOAT_SUFFIXES) {
            FLOAT
        } else {
            DOUBLE
        }),
        INVOCATION if is_size_call(expression, source) => Some(INT),
        UNARY => {
            let negated = expression
                .child_by_field_name(OPERATOR_FIELD)
                .is_some_and(|operator| text(operator, source) == MINUS);
            negated
                .then(|| numeric_type(expression.child_by_field_name(OPERAND_FIELD)?, source))
                .flatten()
        }
        _ => None,
    }
}

// `size()` y `length()` sin argumentos dan un int; IntelliJ no cuenta las llamadas como efectos.
fn is_size_call(call: Node<'_>, source: &[u8]) -> bool {
    call.child_by_field_name(NAME_FIELD)
        .is_some_and(|name| SIZE_METHODS.contains(&text(name, source)))
        && call
            .child_by_field_name(ARGUMENTS_FIELD)
            .is_some_and(|arguments| named_children(arguments).is_empty())
        && call
            .child_by_field_name(OBJECT_FIELD)
            .is_some_and(|receiver| [IDENTIFIER, FIELD_ACCESS].contains(&unparenthesized(receiver).kind()))
}
