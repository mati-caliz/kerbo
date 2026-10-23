use tree_sitter::Node;

use super::java::{NAME_FIELD, text};
use super::java_calls::has_modifier;
use super::java_owner::annotation_names;
use super::{LintFinding, LintKind, finding};
use crate::rules::JavaRule;
use crate::syntax::{descendants, named_children};

const CLASSES: &[&str] = &["class_declaration", "enum_declaration"];
const TOP_LEVEL_TYPES: &[&str] = &[
    "class_declaration",
    "enum_declaration",
    "interface_declaration",
    "record_declaration",
    "annotation_type_declaration",
];
const FIELD: &str = "field_declaration";
const CONSTRUCTOR: &str = "constructor_declaration";
const DECLARATOR: &str = "variable_declarator";
const ASSIGNMENT: &str = "assignment_expression";
const UPDATE: &str = "update_expression";
const IDENTIFIER: &str = "identifier";
const FIELD_ACCESS: &str = "field_access";
const EXPRESSION_STATEMENT: &str = "expression_statement";
const EXPLICIT_CONSTRUCTOR: &str = "explicit_constructor_invocation";
const ENUM_BODY_DECLARATIONS: &str = "enum_body_declarations";
const THIS: &str = "this";
const BODY_FIELD: &str = "body";
const VALUE_FIELD: &str = "value";
const FIELD_FIELD: &str = "field";
const LEFT_FIELD: &str = "left";
const PRIVATE: &str = "private";
const FINAL: &str = "final";
const VOLATILE: &str = "volatile";
const STATIC: &str = "static";
// Lo que escribe el campo por reflexión (inyección, mocks) o le genera un setter.
const IMPLICIT_WRITES: &[&str] = &[
    "Autowired",
    "Inject",
    "Value",
    "Resource",
    "PersistenceContext",
    "Mock",
    "Spy",
    "InjectMocks",
    "Captor",
    "MockBean",
    "SpyBean",
    "MockitoBean",
    "MockitoSpyBean",
    "TempDir",
    "Setter",
];
const GENERATED_SETTERS: &[&str] = &["Data", "Setter"];

// `FieldMayBeFinal` de IntelliJ: un campo privado que se inicializa una sola vez, en la declaración o
// en cada constructor, y no se vuelve a escribir. Un campo privado sólo se escribe desde su tipo de
// nivel superior, así que alcanza con el archivo.
pub fn reported(root: Node<'_>, source: &[u8]) -> Vec<LintFinding> {
    named_children(root)
        .into_iter()
        .filter(|node| TOP_LEVEL_TYPES.contains(&node.kind()))
        .flat_map(|top_level| {
            let writes = writes_in(top_level, source);
            descendants(top_level)
                .into_iter()
                .filter(|node| CLASSES.contains(&node.kind()))
                .flat_map(|class| final_candidates(class, &writes, source))
                .collect::<Vec<_>>()
        })
        .collect()
}

// Una escritura: el nombre del campo y el nodo de la asignación.
struct Write<'tree> {
    name: String,
    node: Node<'tree>,
}

fn writes_in<'tree>(top_level: Node<'tree>, source: &[u8]) -> Vec<Write<'tree>> {
    descendants(top_level)
        .into_iter()
        .filter_map(|node| {
            let target = match node.kind() {
                ASSIGNMENT => node.child_by_field_name(LEFT_FIELD),
                UPDATE => named_children(node).into_iter().next(),
                _ => None,
            }?;
            let name = match target.kind() {
                IDENTIFIER => target,
                FIELD_ACCESS => target.child_by_field_name(FIELD_FIELD)?,
                _ => return None,
            };
            Some(Write {
                name: text(name, source).to_owned(),
                node,
            })
        })
        .collect()
}

fn body_members(class: Node<'_>) -> Vec<Node<'_>> {
    class
        .child_by_field_name(BODY_FIELD)
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .flat_map(|member| {
            if member.kind() == ENUM_BODY_DECLARATIONS {
                named_children(member)
            } else {
                vec![member]
            }
        })
        .collect()
}

fn final_candidates(class: Node<'_>, writes: &[Write<'_>], source: &[u8]) -> Vec<LintFinding> {
    let class_annotations = annotation_names(class, source);
    if class_annotations
        .iter()
        .any(|annotation| GENERATED_SETTERS.contains(&annotation.as_str()))
    {
        return Vec::new();
    }
    let members = body_members(class);
    let constructors: Vec<Node<'_>> = members
        .iter()
        .copied()
        .filter(|member| member.kind() == CONSTRUCTOR)
        .collect();
    members
        .iter()
        .filter(|member| member.kind() == FIELD)
        .filter(|field| {
            has_modifier(**field, PRIVATE, source)
                && !has_modifier(**field, FINAL, source)
                && !has_modifier(**field, VOLATILE, source)
                && !annotation_names(**field, source)
                    .iter()
                    .any(|annotation| IMPLICIT_WRITES.contains(&annotation.as_str()))
        })
        .flat_map(|field| {
            let is_static = has_modifier(*field, STATIC, source);
            named_children(*field)
                .into_iter()
                .filter(|child| child.kind() == DECLARATOR)
                .filter_map(|declarator| {
                    let name = declarator.child_by_field_name(NAME_FIELD)?;
                    let written = text(name, source);
                    let assigned: Vec<&Write<'_>> =
                        writes.iter().filter(|write| write.name == written).collect();
                    let initialized = declarator.child_by_field_name(VALUE_FIELD).is_some();
                    let once = if initialized {
                        assigned.is_empty()
                    } else {
                        !is_static && assigned_once_per_constructor(&constructors, &assigned, source)
                    };
                    once.then(|| finding(LintKind::Java(JavaRule::FieldMayBeFinal), name, written, None))
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

// Cada constructor lo asigna una vez con una sentencia suelta de su cuerpo, o delega en otro con
// `this(...)`; y no se escribe en ningún otro lado.
fn assigned_once_per_constructor(constructors: &[Node<'_>], assigned: &[&Write<'_>], source: &[u8]) -> bool {
    if constructors.is_empty() {
        return false;
    }
    let mut expected = 0;
    for constructor in constructors {
        let statements = constructor
            .child_by_field_name(BODY_FIELD)
            .map(named_children)
            .unwrap_or_default();
        let delegates = statements.first().is_some_and(|first| {
            first.kind() == EXPLICIT_CONSTRUCTOR
                && first
                    .child(0)
                    .is_some_and(|keyword| text(keyword, source) == THIS)
        });
        if delegates {
            continue;
        }
        let direct = statements
            .iter()
            .filter(|statement| statement.kind() == EXPRESSION_STATEMENT)
            .filter_map(|statement| named_children(*statement).into_iter().next())
            .filter(|expression| assigned.iter().any(|write| write.node == *expression))
            .count();
        if direct != 1 {
            return false;
        }
        expected += 1;
    }
    assigned.len() == expected
}
