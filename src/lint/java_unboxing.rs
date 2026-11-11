use tree_sitter::Node;

use super::java::text;
use crate::syntax::one_based;

const PARENTHESIZED: &str = "parenthesized_expression";
const RETURN: &str = "return_statement";
const LAMBDA: &str = "lambda_expression";
const METHOD: &str = "method_declaration";
const VARIABLE_DECLARATOR: &str = "variable_declarator";
const TYPE_FIELD: &str = "type";
const VOID: &str = "void";
const PRIMITIVE_TYPES: &[&str] = &["integral_type", "floating_point_type", "boolean_type"];

// Si el valor de la llamada se desempaqueta: lo devuelve un método que devuelve un primitivo, o
// inicializa una variable primitiva.
pub(super) fn unboxed_at(invocation: Node<'_>, source: &[u8]) -> Option<(usize, usize)> {
    let mut parent = invocation.parent()?;
    while parent.kind() == PARENTHESIZED {
        parent = parent.parent()?;
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
        _ => return None,
    };
    let primitive = PRIMITIVE_TYPES.contains(&declared_type.kind()) && text(declared_type, source) != VOID;
    let start = invocation.start_position();
    primitive.then(|| (one_based(start.row), one_based(start.column)))
}
