use tree_sitter::Node;

use super::java::text;
use crate::syntax::one_based;

const PARENTHESIZED: &str = "parenthesized_expression";
const RETURN: &str = "return_statement";
const LAMBDA: &str = "lambda_expression";
const METHOD: &str = "method_declaration";
const VARIABLE_DECLARATOR: &str = "variable_declarator";
const CAST: &str = "cast_expression";
const BINARY: &str = "binary_expression";
const OPERATOR_FIELD: &str = "operator";
const UNBOXING_OPERATORS: &[&str] = &["-", "*", "/", "%", "^", "<<", ">>", ">>>", "<", ">", "<=", ">="];
const TYPE_FIELD: &str = "type";
const VOID: &str = "void";
const PRIMITIVE_TYPES: &[&str] = &["integral_type", "floating_point_type", "boolean_type"];

// Si el valor de la llamada se desempaqueta: lo devuelve un método que devuelve un primitivo,
// inicializa una variable primitiva, se castea a un primitivo (`(int) cola.peek()`) o es operando de
// una cuenta (`cola.poll() ^ 1`; no `+`, que puede ser concatenar, ni `==`).
pub(super) fn unboxed_at(invocation: Node<'_>, source: &[u8]) -> Option<(usize, usize)> {
    let mut parent = invocation.parent()?;
    while parent.kind() == PARENTHESIZED {
        parent = parent.parent()?;
    }
    let start = invocation.start_position();
    let position = (one_based(start.row), one_based(start.column));
    if parent.kind() == BINARY {
        let operator = parent.child_by_field_name(OPERATOR_FIELD)?;
        return UNBOXING_OPERATORS
            .contains(&text(operator, source))
            .then_some(position);
    }
    let declared_type = match parent.kind() {
        RETURN => {
            let mut current = parent.parent();
            while let Some(node) = current {
                if node.kind() == LAMBDA {
                    return None;
                }
                if node.kind() == METHOD {
                    break;
                }
                current = node.parent();
            }
            current?.child_by_field_name(TYPE_FIELD)?
        }
        VARIABLE_DECLARATOR => parent.parent()?.child_by_field_name(TYPE_FIELD)?,
        CAST => parent.child_by_field_name(TYPE_FIELD)?,
        _ => return None,
    };
    let primitive = PRIMITIVE_TYPES.contains(&declared_type.kind()) && text(declared_type, source) != VOID;
    primitive.then_some(position)
}
