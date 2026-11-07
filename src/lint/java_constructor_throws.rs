use tree_sitter::Node;

use super::java::{has_modifier, is_type_declaration, text};
use super::java_chains::argument_count;
use super::java_receivers::Variables;
use super::java_throws::{ThrowOrigin, ThrowPoint, throw_points, written_type};
use crate::syntax::named_children;

const EXPLICIT_CONSTRUCTOR: &str = "explicit_constructor_invocation";
const CONSTRUCTOR_FIELD: &str = "constructor";
const OBJECT_FIELD: &str = "object";
const NAME_FIELD: &str = "name";
const BODY_FIELD: &str = "body";
const VALUE_FIELD: &str = "value";
const SUPERCLASS_FIELD: &str = "superclass";
const THIS: &str = "this";
const FIELD_DECLARATION: &str = "field_declaration";
const VARIABLE_DECLARATOR: &str = "variable_declarator";
const INSTANCE_INITIALIZER: &str = "block";
const STATIC: &str = "static";
const NO_ARGUMENTS: usize = 0;
const OBJECT: &str = "Object";
const CREATION: &str = "object_creation_expression";
const CLASS_BODY: &str = "class_body";
const TYPE_FIELD: &str = "type";

// Lo que tira un constructor: su cuerpo, el `super()` implícito si no llama a otro, y los
// inicializadores de los campos y los bloques de instancia, que corren adentro de él salvo que
// delegue con `this(...)`.
pub fn constructor_points(
    constructor: Node<'_>,
    body: Node<'_>,
    source: &[u8],
    variables: &Variables,
) -> Vec<ThrowPoint> {
    let mut points = throw_points(body, source, variables);
    let Some(class) = enclosing_class(constructor) else {
        return vec![unknown_point()];
    };
    let explicit = named_children(body)
        .into_iter()
        .find(|statement| statement.kind() == EXPLICIT_CONSTRUCTOR);
    if explicit.is_some_and(|invocation| delegates_to_this(invocation)) {
        return points;
    }
    if explicit.is_none() {
        points.extend(superclass_of(class, source).map(|superclass| ThrowPoint {
            origin: ThrowOrigin::Creation(superclass, NO_ARGUMENTS),
            caught: Vec::new(),
        }));
    }
    for initializer in instance_initializers(class) {
        points.extend(throw_points(initializer, source, variables));
    }
    points
}

// `this(...)` va a otro constructor de la clase y `super(...)` a uno del padre; `afuera.super(...)`
// no se sigue.
pub fn explicit_origin(invocation: Node<'_>, source: &[u8]) -> Option<ThrowOrigin> {
    if invocation.child_by_field_name(OBJECT_FIELD).is_some() {
        return Some(ThrowOrigin::Unknown);
    }
    let class = enclosing_class(invocation)?;
    let arity = argument_count(invocation);
    if delegates_to_this(invocation) {
        let name = class.child_by_field_name(NAME_FIELD)?;
        return Some(ThrowOrigin::Creation(text(name, source).to_owned(), arity));
    }
    superclass_of(class, source).map(|superclass| ThrowOrigin::Creation(superclass, arity))
}

fn delegates_to_this(invocation: Node<'_>) -> bool {
    invocation
        .child_by_field_name(CONSTRUCTOR_FIELD)
        .is_some_and(|keyword| keyword.kind() == THIS)
}

// La clase de arriba de la que encierra a `super`: la que extiende, `Object` si no extiende nada, o
// la que crea una clase anónima.
pub fn super_type(keyword: Node<'_>, source: &[u8]) -> Option<String> {
    let mut current = keyword.parent();
    while let Some(candidate) = current {
        if is_type_declaration(candidate) {
            return Some(superclass_of(candidate, source).unwrap_or_else(|| OBJECT.to_owned()));
        }
        if candidate.kind() == CREATION
            && named_children(candidate)
                .iter()
                .any(|child| child.kind() == CLASS_BODY)
        {
            let created = candidate.child_by_field_name(TYPE_FIELD)?;
            return Some(written_type(text(created, source)));
        }
        current = candidate.parent();
    }
    None
}

fn enclosing_class(node: Node<'_>) -> Option<Node<'_>> {
    let mut current = node.parent();
    while let Some(candidate) = current {
        if is_type_declaration(candidate) {
            return Some(candidate);
        }
        current = candidate.parent();
    }
    None
}

fn superclass_of(class: Node<'_>, source: &[u8]) -> Option<String> {
    let clause = class.child_by_field_name(SUPERCLASS_FIELD)?;
    let written = named_children(clause).into_iter().next()?;
    Some(written_type(text(written, source)))
}

fn instance_initializers(class: Node<'_>) -> Vec<Node<'_>> {
    let Some(body) = class.child_by_field_name(BODY_FIELD) else {
        return Vec::new();
    };
    named_children(body)
        .into_iter()
        .flat_map(|member| match member.kind() {
            INSTANCE_INITIALIZER => vec![member],
            FIELD_DECLARATION if !has_modifier(member, STATIC) => named_children(member)
                .into_iter()
                .filter(|child| child.kind() == VARIABLE_DECLARATOR)
                .filter_map(|declarator| declarator.child_by_field_name(VALUE_FIELD))
                .collect(),
            _ => Vec::new(),
        })
        .collect()
}

const fn unknown_point() -> ThrowPoint {
    ThrowPoint {
        origin: ThrowOrigin::Unknown,
        caught: Vec::new(),
    }
}
