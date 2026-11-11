use std::collections::HashMap;

use tree_sitter::Node;

use super::java::{ARRAY_CREATION, IDENTIFIER, VARIABLE_DECLARATOR, text};
use super::java_inference::is_member_name;
use super::{LintFinding, LintKind, finding};
use crate::rules::JavaRule;
use crate::syntax::{descendants, named_children};

const BLOCKS: &[&str] = &["block", "constructor_body"];
const COMMENTS: &[&str] = &["line_comment", "block_comment"];
const DECLARATION: &str = "local_variable_declaration";
const EXPRESSION_STATEMENT: &str = "expression_statement";
const ASSIGNMENT: &str = "assignment_expression";
const ARRAY_ACCESS: &str = "array_access";
const FIELD_ACCESS: &str = "field_access";
const INVOCATION: &str = "method_invocation";
const DIMENSIONS_EXPRESSION: &str = "dimensions_expr";
const INTEGER: &str = "decimal_integer_literal";
const LITERALS: &[&str] = &["true", "false", "null_literal", INTEGER];
const DEFERRED_CODE: &[&str] = &["lambda_expression", "class_body"];
const TYPE_FIELD: &str = "type";
const NAME_FIELD: &str = "name";
const VALUE_FIELD: &str = "value";
const DIMENSIONS_FIELD: &str = "dimensions";
const LEFT_FIELD: &str = "left";
const RIGHT_FIELD: &str = "right";
const OPERATOR_FIELD: &str = "operator";
const ARRAY_FIELD: &str = "array";
const INDEX_FIELD: &str = "index";
const OBJECT_FIELD: &str = "object";
const PLAIN_ASSIGNMENT: &str = "=";
const ARRAY_BRACKET: char = '[';
const BOOLEAN_DEFAULT: &str = "false";
const INTEGRAL_DEFAULT: &str = "0";
const REFERENCE_DEFAULT: &str = "null";
const INTEGRAL_TYPES: &[&str] = &["byte", "short", "int", "long"];
const UNTRACKED_PRIMITIVES: &[&str] = &["char", "float", "double"];
const BOOLEAN_TYPE: &str = "boolean";

type IndexPath = Vec<String>;

// Lo que se sabe de una variable local del bloque: su valor (con camino vacío) o el de cada
// posición de un arreglo recién creado, y el valor por defecto de un arreglo de una dimensión
// con largo literal (`new boolean[2]` arranca en `false`).
struct Tracked {
    dimensions: usize,
    values: HashMap<IndexPath, String>,
    default: Option<(String, usize)>,
}

impl Tracked {
    fn known(&self, path: &IndexPath) -> Option<&str> {
        self.values.get(path).map(String::as_str).or_else(|| {
            let (value, length) = self.default.as_ref()?;
            let [index] = path.as_slice() else {
                return None;
            };
            (index.parse::<usize>().ok()? < *length).then_some(value.as_str())
        })
    }

    fn forget(&mut self) {
        self.values.clear();
        self.default = None;
    }
}

struct ConstantWrite<'tree> {
    target: Node<'tree>,
    name: String,
    path: IndexPath,
    value: String,
}

// Calca "Variable is already assigned to this value" de DataFlowIssue en el caso de línea
// recta: dentro de un bloque, asignar un literal a una variable local (o a una posición fija de
// un arreglo local recién creado) que ya tiene ese valor, sin que nada la toque en el medio.
pub fn reported(nodes: &[Node<'_>], source: &[u8]) -> Vec<LintFinding> {
    nodes
        .iter()
        .filter(|node| BLOCKS.contains(&node.kind()))
        .flat_map(|block| in_block(*block, source))
        .collect()
}

fn in_block(block: Node<'_>, source: &[u8]) -> Vec<LintFinding> {
    let mut tracked: HashMap<String, Tracked> = HashMap::new();
    let mut found = Vec::new();
    let statements = named_children(block)
        .into_iter()
        .filter(|child| !COMMENTS.contains(&child.kind()));
    for statement in statements {
        if let Some(write) = constant_write(statement, source)
            && let Some(variable) = tracked.get_mut(&write.name)
            && variable.dimensions == write.path.len()
        {
            if variable.known(&write.path) == Some(write.value.as_str()) {
                found.push(finding(
                    LintKind::Java(JavaRule::AlreadyAssigned),
                    statement,
                    text(write.target, source),
                    None,
                ));
            }
            variable.values.insert(write.path, write.value);
            continue;
        }
        forget_mentions(statement, source, &mut tracked);
        if statement.kind() == DECLARATION {
            tracked.extend(declared(statement, source));
        }
    }
    found
}

fn constant_write<'tree>(statement: Node<'tree>, source: &[u8]) -> Option<ConstantWrite<'tree>> {
    if statement.kind() != EXPRESSION_STATEMENT {
        return None;
    }
    let assignment = named_children(statement)
        .into_iter()
        .find(|child| child.kind() == ASSIGNMENT)?;
    let operator = assignment.child_by_field_name(OPERATOR_FIELD)?;
    let value = assignment.child_by_field_name(RIGHT_FIELD)?;
    if text(operator, source) != PLAIN_ASSIGNMENT || !LITERALS.contains(&value.kind()) {
        return None;
    }
    let target = assignment.child_by_field_name(LEFT_FIELD)?;
    let mut path = Vec::new();
    let mut base = target;
    while base.kind() == ARRAY_ACCESS {
        let index = base.child_by_field_name(INDEX_FIELD)?;
        if index.kind() != INTEGER {
            return None;
        }
        path.push(text(index, source).to_owned());
        base = base.child_by_field_name(ARRAY_FIELD)?;
    }
    path.reverse();
    (base.kind() == IDENTIFIER).then(|| ConstantWrite {
        target,
        name: text(base, source).to_owned(),
        path,
        value: text(value, source).to_owned(),
    })
}

// Cualquier otro uso de una variable seguida olvida lo que se sabía de ella; si el arreglo (o una
// fila suya) queda en otro lado, o lo usa una lambda, deja de seguirse.
fn forget_mentions(statement: Node<'_>, source: &[u8], tracked: &mut HashMap<String, Tracked>) {
    for identifier in descendants(statement) {
        if identifier.kind() != IDENTIFIER || is_member_name(identifier) {
            continue;
        }
        let name = text(identifier, source);
        let Some(variable) = tracked.get_mut(name) else {
            continue;
        };
        if escapes(identifier, variable.dimensions, statement) {
            tracked.remove(name);
        } else {
            variable.forget();
        }
    }
}

fn escapes(identifier: Node<'_>, dimensions: usize, statement: Node<'_>) -> bool {
    let mut ancestor = identifier.parent();
    while let Some(node) = ancestor.filter(|node| node.id() != statement.id()) {
        if DEFERRED_CODE.contains(&node.kind()) {
            return true;
        }
        ancestor = node.parent();
    }
    let mut accessed = identifier;
    let mut depth = 0;
    while let Some(parent) = accessed
        .parent()
        .filter(|parent| is_child_in(*parent, ARRAY_ACCESS, ARRAY_FIELD, accessed))
    {
        accessed = parent;
        depth += 1;
    }
    let Some(parent) = accessed.parent() else {
        return false;
    };
    depth < dimensions
        && !is_child_in(parent, FIELD_ACCESS, OBJECT_FIELD, accessed)
        && !is_child_in(parent, INVOCATION, OBJECT_FIELD, accessed)
        && !is_child_in(parent, ASSIGNMENT, LEFT_FIELD, accessed)
}

fn is_child_in(parent: Node<'_>, kind: &str, field: &str, child: Node<'_>) -> bool {
    parent.kind() == kind
        && parent
            .child_by_field_name(field)
            .is_some_and(|node| node.id() == child.id())
}

fn declared(declaration: Node<'_>, source: &[u8]) -> Vec<(String, Tracked)> {
    let Some(declared_type) = declaration.child_by_field_name(TYPE_FIELD) else {
        return Vec::new();
    };
    let type_text = text(declared_type, source);
    let dimensions = type_text.matches(ARRAY_BRACKET).count();
    let element_type = type_text.split(ARRAY_BRACKET).next().unwrap_or_default().trim();
    named_children(declaration)
        .into_iter()
        .filter(|child| child.kind() == VARIABLE_DECLARATOR)
        .filter(|declarator| declarator.child_by_field_name(DIMENSIONS_FIELD).is_none())
        .filter_map(|declarator| {
            let name = text(declarator.child_by_field_name(NAME_FIELD)?, source).to_owned();
            let value = declarator.child_by_field_name(VALUE_FIELD);
            let tracked = if dimensions == 0 {
                scalar(value, source)
            } else {
                fresh_array(value?, dimensions, element_type, source)?
            };
            Some((name, tracked))
        })
        .collect()
}

fn scalar(value: Option<Node<'_>>, source: &[u8]) -> Tracked {
    let values = value
        .filter(|value| LITERALS.contains(&value.kind()))
        .map(|literal| (Vec::new(), text(literal, source).to_owned()))
        .into_iter()
        .collect();
    Tracked {
        dimensions: 0,
        values,
        default: None,
    }
}

fn fresh_array(creation: Node<'_>, dimensions: usize, element_type: &str, source: &[u8]) -> Option<Tracked> {
    if creation.kind() != ARRAY_CREATION {
        return None;
    }
    let sizes: Vec<Node<'_>> = named_children(creation)
        .into_iter()
        .filter(|child| child.kind() == DIMENSIONS_EXPRESSION)
        .collect();
    let default = match sizes.as_slice() {
        [size] if dimensions == 1 => {
            let length = named_children(*size)
                .into_iter()
                .find(|child| child.kind() == INTEGER)
                .and_then(|literal| text(literal, source).parse().ok());
            default_value(element_type).zip(length)
        }
        _ => None,
    };
    Some(Tracked {
        dimensions,
        values: HashMap::new(),
        default,
    })
}

fn default_value(element_type: &str) -> Option<String> {
    let value = if element_type == BOOLEAN_TYPE {
        BOOLEAN_DEFAULT
    } else if INTEGRAL_TYPES.contains(&element_type) {
        INTEGRAL_DEFAULT
    } else if UNTRACKED_PRIMITIVES.contains(&element_type) {
        return None;
    } else {
        REFERENCE_DEFAULT
    };
    Some(value.to_owned())
}
