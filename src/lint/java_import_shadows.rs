use tree_sitter::Node;

use super::imports::javadoc;
use super::java::{NAME_FIELD, TYPE_DECLARATIONS, children, qualified_name, text};
use super::java_receivers::{simple_type_name, supertypes};
use crate::syntax::{descendants, named_children, one_based};

const IMPORT: &str = "import_declaration";
const STATIC: &str = "static";
const ASTERISK: &str = "asterisk";
const OBJECT_CREATION: &str = "object_creation_expression";
const CLASS_BODY: &str = "class_body";
const TYPE_IDENTIFIER: &str = "type_identifier";
const IDENTIFIER: &str = "identifier";
const BODY_FIELD: &str = "body";
const TYPE_FIELD: &str = "type";
const SEPARATOR: char = '.';

// Una clase que encierra un uso: su nombre (ninguno si es anónima) y los supertipos escritos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Enclosing {
    pub name: Option<String>,
    pub supertypes: Vec<String>,
}

// Un import de un tipo anidado (`java.util.Map.Entry`) que el archivo usa fuera de los Javadoc (PMD
// cuenta una mención en un Javadoc como uso), con las clases que encierran cada uso. PMD lo da por innecesario si en todos los usos el nombre ya llega como tipo
// miembro heredado (`Entry` dentro de una clase que extiende `HashMap`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportShadow {
    pub imported: String,
    pub line: usize,
    pub column: usize,
    pub usages: Vec<Vec<Enclosing>>,
}

pub fn import_shadows(root: Node<'_>, source: &[u8]) -> Vec<ImportShadow> {
    let nodes = descendants(root);
    let documented = javadoc::referenced_names(root, source);
    named_children(root)
        .into_iter()
        .filter(|declaration| declaration.kind() == IMPORT)
        .filter(|declaration| {
            !children(*declaration)
                .iter()
                .any(|child| child.kind() == STATIC || child.kind() == ASTERISK)
        })
        .filter_map(|declaration| {
            let imported = qualified_name(declaration, source);
            let (outer, simple) = imported.rsplit_once(SEPARATOR)?;
            let nested = outer
                .rsplit(SEPARATOR)
                .next()
                .is_some_and(|owner| owner.starts_with(|first: char| first.is_ascii_uppercase()));
            if !nested || documented.contains(simple) {
                return None;
            }
            let usages: Vec<Vec<Enclosing>> = nodes
                .iter()
                .filter(|node| {
                    (node.kind() == TYPE_IDENTIFIER || node.kind() == IDENTIFIER)
                        && text(**node, source) == simple
                        && !inside_import(**node)
                })
                .map(|usage| enclosing_classes(*usage, source))
                .collect();
            let start = named_children(declaration)
                .into_iter()
                .next()
                .map_or_else(|| declaration.start_position(), |name| name.start_position());
            (!usages.is_empty()).then(|| ImportShadow {
                imported: imported.clone(),
                line: one_based(start.row),
                column: one_based(start.column),
                usages,
            })
        })
        .collect()
}

// Los tipos anidados que declara cada tipo del archivo: (el de afuera, el de adentro).
pub fn nested_types(root: Node<'_>, source: &[u8]) -> Vec<(String, String)> {
    descendants(root)
        .into_iter()
        .filter(|node| TYPE_DECLARATIONS.contains(&node.kind()))
        .filter_map(|inner| {
            let owner = inner
                .parent()?
                .parent()
                .filter(|owner| TYPE_DECLARATIONS.contains(&owner.kind()))?;
            Some((
                text(owner.child_by_field_name(NAME_FIELD)?, source).to_owned(),
                text(inner.child_by_field_name(NAME_FIELD)?, source).to_owned(),
            ))
        })
        .collect()
}

fn inside_import(node: Node<'_>) -> bool {
    std::iter::successors(node.parent(), Node::parent).any(|ancestor| ancestor.kind() == IMPORT)
}

// Las clases cuyo cuerpo encierra el uso, de la más cercana a la de afuera: un uso en el `extends`
// de una clase se resuelve en la que la encierra.
fn enclosing_classes(usage: Node<'_>, source: &[u8]) -> Vec<Enclosing> {
    let mut found = Vec::new();
    let mut child = usage;
    for ancestor in std::iter::successors(usage.parent(), Node::parent) {
        let in_body = ancestor
            .child_by_field_name(BODY_FIELD)
            .is_some_and(|body| body.id() == child.id());
        if TYPE_DECLARATIONS.contains(&ancestor.kind()) && in_body {
            found.push(Enclosing {
                name: ancestor
                    .child_by_field_name(NAME_FIELD)
                    .map(|name| text(name, source).to_owned()),
                supertypes: supertypes(ancestor, source),
            });
        } else if ancestor.kind() == OBJECT_CREATION && child.kind() == CLASS_BODY {
            found.push(Enclosing {
                name: None,
                supertypes: ancestor
                    .child_by_field_name(TYPE_FIELD)
                    .map(|created| vec![simple_type_name(text(created, source))])
                    .unwrap_or_default(),
            });
        }
        child = ancestor;
    }
    found
}
