use tree_sitter::Node;

use super::java::{NAME_FIELD, text};
use super::java_calls::TYPE_DECLARATIONS;
use super::java_receivers::{Variables, simple_type_name};
use super::java_throws::{ThrowOrigin, call_origin, caught_types};
use crate::syntax::named_children;

const INVOCATION: &str = "method_invocation";
const CREATION: &str = "object_creation_expression";
const METHOD_REFERENCE: &str = "method_reference";
const CLASS_BODY: &str = "class_body";
const TRY_STATEMENTS: &[&str] = &["try_statement", "try_with_resources_statement"];
const CATCH_CLAUSE: &str = "catch_clause";
const TYPE_FIELD: &str = "type";

// Una llamada adentro de un `try`, con los tipos de sus `catch` y por dónde se sabe a qué método va.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaughtCall {
    pub name: String,
    pub caught: Vec<String>,
    pub origin: ThrowOrigin,
}

// Una llamada o referencia a un método que queda adentro de algún `try` de su clase: IntelliJ no
// marca un `throws` que un llamador atrapa con un `catch` de ese tipo.
pub fn caught_call(node: Node<'_>, source: &[u8], variables: &Variables) -> Option<CaughtCall> {
    let (name, origin) = match node.kind() {
        INVOCATION => (
            node.child_by_field_name(NAME_FIELD)?,
            call_origin(node, source, variables),
        ),
        METHOD_REFERENCE => (*named_children(node).last()?, ThrowOrigin::Unknown),
        CREATION => (node.child_by_field_name(TYPE_FIELD)?, ThrowOrigin::Unknown),
        _ => return None,
    };
    let caught = surrounding_catches(node, source);
    (!caught.is_empty()).then(|| CaughtCall {
        name: simple_type_name(text(name, source)),
        caught,
        origin,
    })
}

fn surrounding_catches(node: Node<'_>, source: &[u8]) -> Vec<String> {
    let mut caught = Vec::new();
    let mut current = node.parent();
    while let Some(ancestor) = current {
        if TYPE_DECLARATIONS.contains(&ancestor.kind()) || ancestor.kind() == CLASS_BODY {
            break;
        }
        if TRY_STATEMENTS.contains(&ancestor.kind()) {
            caught.extend(
                named_children(ancestor)
                    .into_iter()
                    .filter(|part| part.kind() == CATCH_CLAUSE)
                    .flat_map(|clause| caught_types(clause, source)),
            );
        }
        current = ancestor.parent();
    }
    caught
}
