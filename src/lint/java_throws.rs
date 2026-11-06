use std::rc::Rc;

use tree_sitter::Node;

use super::java::{METHOD, NAME_FIELD, text};
use super::java_calls::TYPE_DECLARATIONS;
use super::java_chains::argument_count;
use super::java_receivers::{Variables, enclosing_types, simple_type_name};
use crate::syntax::{descendants, named_children, one_based};

const THROWS: &str = "throws";
const INVOCATION: &str = "method_invocation";
const CREATION: &str = "object_creation_expression";
const EXPLICIT_CONSTRUCTOR: &str = "explicit_constructor_invocation";
const THROW: &str = "throw_statement";
const RESOURCE: &str = "resource";
const LAMBDA: &str = "lambda_expression";
const METHOD_REFERENCE: &str = "method_reference";
const CLASS_BODY: &str = "class_body";
const TRY_STATEMENTS: &[&str] = &["try_statement", "try_with_resources_statement"];
const CATCH_CLAUSE: &str = "catch_clause";
const CATCH_PARAMETER: &str = "catch_formal_parameter";
const CATCH_TYPE: &str = "catch_type";
const FINALLY: &str = "finally_clause";
const IDENTIFIER: &str = "identifier";
const FIELD_ACCESS: &str = "field_access";
const STRING_LITERAL: &str = "string_literal";
const CLASS_LITERAL: &str = "class_literal";
const CLASS_TYPE: &str = "Class";
const THIS: &str = "this";
const STRING_TYPE: &str = "String";
const INFERRED: &str = "var";
const BODY_FIELD: &str = "body";
const TYPE_FIELD: &str = "type";
const OBJECT_FIELD: &str = "object";
const FIELD_FIELD: &str = "field";
const GENERIC_OPEN: char = '<';
const ARRAY_OPEN: char = '[';
const QUALIFIER_SEPARATOR: char = '.';

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThrowsClause {
    pub written: String,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallRoot {
    Scope(Vec<String>),
    Type(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallStep {
    pub name: String,
    pub arity: usize,
}

// Lo que en un cuerpo puede tirar una excepción chequeada. `Unknown` es lo que no se sabe sin
// compilar: como IntelliJ con una llamada sin resolver, alcanza para no opinar sobre el método.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThrowOrigin {
    Unknown,
    Thrown(String),
    Creation(String, usize),
    Close(String),
    Call(CallRoot, Vec<CallStep>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThrowPoint {
    pub origin: ThrowOrigin,
    pub caught: Vec<String>,
}

// La cláusula `throws` de un método, ubicado por la posición de su nombre, y lo que tira su cuerpo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodThrows {
    pub line: usize,
    pub column: usize,
    pub clauses: Vec<ThrowsClause>,
    pub points: Vec<ThrowPoint>,
}

// Una llamada adentro de un `try`, con los tipos de sus `catch`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaughtCall {
    pub name: String,
    pub caught: Vec<String>,
}

pub fn method_throws(method: Node<'_>, source: &[u8], variables: &Variables) -> Option<MethodThrows> {
    let clause_list = named_children(method)
        .into_iter()
        .find(|child| child.kind() == THROWS)?;
    let name = method.child_by_field_name(NAME_FIELD)?.start_position();
    let points = method
        .child_by_field_name(BODY_FIELD)
        .filter(|_| method.kind() == METHOD)
        .map(|body| throw_points(body, source, &with_catch_parameters(body, source, variables)))
        .unwrap_or_default();
    Some(MethodThrows {
        line: one_based(name.row),
        column: one_based(name.column),
        clauses: named_children(clause_list)
            .into_iter()
            .map(|written| clause_of(written, source))
            .collect(),
        points,
    })
}

// La variable de un `catch` tiene su tipo, salvo en `catch (A | B e)` o si el nombre se repite con otro.
fn with_catch_parameters(body: Node<'_>, source: &[u8], variables: &Variables) -> Variables {
    let mut scoped = variables.clone();
    for parameter in descendants(body)
        .into_iter()
        .filter(|node| node.kind() == CATCH_PARAMETER)
    {
        let Some(name) = parameter.child_by_field_name(NAME_FIELD) else {
            continue;
        };
        let types = catch_parameter_types(parameter, source);
        let declared = match types.as_slice() {
            [single] => Some(simple_type_name(single)),
            _ => None,
        };
        scoped
            .entry(text(name, source).to_owned())
            .and_modify(|known| {
                if *known != declared {
                    *known = None;
                }
            })
            .or_insert(declared);
    }
    scoped
}

fn clause_of(written: Node<'_>, source: &[u8]) -> ThrowsClause {
    let start = written.start_position();
    ThrowsClause {
        written: written_type(text(written, source)),
        line: one_based(start.row),
        column: one_based(start.column),
    }
}

fn written_type(written: &str) -> String {
    let without_arguments = written.split(GENERIC_OPEN).next().unwrap_or(written);
    without_arguments
        .split(ARRAY_OPEN)
        .next()
        .unwrap_or(without_arguments)
        .trim()
        .to_owned()
}

// Una lambda o una clase anónima no tiran al declararse: lo de adentro corre en otro método.
fn throw_points(body: Node<'_>, source: &[u8], variables: &Variables) -> Vec<ThrowPoint> {
    let mut pending: Vec<(Node<'_>, Rc<[String]>)> = vec![(body, Rc::from(Vec::new()))];
    let mut points = Vec::new();
    while let Some((node, caught)) = pending.pop() {
        let kind = node.kind();
        if kind == LAMBDA || kind == METHOD_REFERENCE || TYPE_DECLARATIONS.contains(&kind) {
            continue;
        }
        if TRY_STATEMENTS.contains(&kind) {
            pending.extend(try_parts(node, source, &caught));
            continue;
        }
        if let Some(origin) = origin_of(node, source, variables) {
            points.push(ThrowPoint {
                origin,
                caught: caught.to_vec(),
            });
        }
        pending.extend(
            named_children(node)
                .into_iter()
                .filter(|child| child.kind() != CLASS_BODY)
                .map(|child| (child, Rc::clone(&caught))),
        );
    }
    points
}

// El bloque y los recursos están cubiertos por los `catch` de ese `try`; los `catch` y el `finally`,
// sólo por los de afuera.
fn try_parts<'tree>(
    statement: Node<'tree>,
    source: &[u8],
    outer: &Rc<[String]>,
) -> Vec<(Node<'tree>, Rc<[String]>)> {
    let parts = named_children(statement);
    let mut covering = outer.to_vec();
    for clause in parts.iter().filter(|part| part.kind() == CATCH_CLAUSE) {
        covering.extend(caught_types(*clause, source));
    }
    let inner: Rc<[String]> = Rc::from(covering);
    parts
        .into_iter()
        .map(|part| {
            if part.kind() == CATCH_CLAUSE || part.kind() == FINALLY {
                (part, Rc::clone(outer))
            } else {
                (part, Rc::clone(&inner))
            }
        })
        .collect()
}

fn caught_types(clause: Node<'_>, source: &[u8]) -> Vec<String> {
    named_children(clause)
        .into_iter()
        .filter(|child| child.kind() == CATCH_PARAMETER)
        .flat_map(|parameter| catch_parameter_types(parameter, source))
        .collect()
}

fn catch_parameter_types(parameter: Node<'_>, source: &[u8]) -> Vec<String> {
    named_children(parameter)
        .into_iter()
        .filter(|child| child.kind() == CATCH_TYPE)
        .flat_map(named_children)
        .map(|written| written_type(text(written, source)))
        .collect()
}

fn origin_of(node: Node<'_>, source: &[u8], variables: &Variables) -> Option<ThrowOrigin> {
    match node.kind() {
        INVOCATION => (!is_chain_receiver(node)).then(|| call_origin(node, source, variables)),
        CREATION => Some(creation_origin(node, source)),
        THROW => Some(thrown_origin(node, source)),
        RESOURCE => Some(close_origin(node, source, variables)),
        EXPLICIT_CONSTRUCTOR => Some(ThrowOrigin::Unknown),
        _ => None,
    }
}

// En `a.b().c()` la llamada a `b` se mira como parte de la cadena que termina en `c`.
fn is_chain_receiver(invocation: Node<'_>) -> bool {
    invocation
        .parent()
        .filter(|parent| parent.kind() == INVOCATION)
        .and_then(|parent| parent.child_by_field_name(OBJECT_FIELD))
        .is_some_and(|object| object.id() == invocation.id())
}

fn call_origin(invocation: Node<'_>, source: &[u8], variables: &Variables) -> ThrowOrigin {
    let mut steps = Vec::new();
    let mut current = invocation;
    loop {
        let Some(name) = current.child_by_field_name(NAME_FIELD) else {
            return ThrowOrigin::Unknown;
        };
        steps.push(CallStep {
            name: text(name, source).to_owned(),
            arity: argument_count(current),
        });
        let Some(object) = current.child_by_field_name(OBJECT_FIELD) else {
            steps.reverse();
            return ThrowOrigin::Call(CallRoot::Scope(enclosing_types(current, source)), steps);
        };
        if object.kind() != INVOCATION {
            steps.reverse();
            return root_of(object, source, variables)
                .map_or(ThrowOrigin::Unknown, |root| ThrowOrigin::Call(root, steps));
        }
        current = object;
    }
}

fn root_of(object: Node<'_>, source: &[u8], variables: &Variables) -> Option<CallRoot> {
    match object.kind() {
        THIS => Some(CallRoot::Scope(enclosing_types(object, source))),
        IDENTIFIER => identifier_root(text(object, source), variables),
        STRING_LITERAL => Some(CallRoot::Type(STRING_TYPE.to_owned())),
        CLASS_LITERAL => Some(CallRoot::Type(CLASS_TYPE.to_owned())),
        CREATION => object
            .child_by_field_name(TYPE_FIELD)
            .map(|written| CallRoot::Type(written_type(text(written, source)))),
        FIELD_ACCESS => field_root(object, source, variables),
        _ => None,
    }
}

fn identifier_root(name: &str, variables: &Variables) -> Option<CallRoot> {
    match variables.get(name) {
        Some(Some(declared)) => Some(CallRoot::Type(declared.clone())),
        None if name.starts_with(|first: char| first.is_ascii_uppercase()) => {
            Some(CallRoot::Type(name.to_owned()))
        }
        Some(None) | None => None,
    }
}

// `this.campo` tiene el tipo del campo, y `a.b.Clase` o `Afuera.Adentro` son una clase.
fn field_root(access: Node<'_>, source: &[u8], variables: &Variables) -> Option<CallRoot> {
    if access
        .child_by_field_name(OBJECT_FIELD)
        .is_some_and(|owner| owner.kind() == THIS)
    {
        let field = access.child_by_field_name(FIELD_FIELD)?;
        return identifier_root(text(field, source), variables);
    }
    let written = text(access, source);
    written
        .rsplit(QUALIFIER_SEPARATOR)
        .next()
        .is_some_and(|last| last.starts_with(|first: char| first.is_ascii_uppercase()))
        .then(|| CallRoot::Type(written.to_owned()))
}

fn creation_origin(creation: Node<'_>, source: &[u8]) -> ThrowOrigin {
    creation
        .child_by_field_name(TYPE_FIELD)
        .map_or(ThrowOrigin::Unknown, |written| {
            ThrowOrigin::Creation(written_type(text(written, source)), argument_count(creation))
        })
}

// `throw e` puede ser el relanzamiento preciso de un `catch`, que pide flujo de datos para saber qué
// tipo lleva: sólo se entiende el `throw new`.
fn thrown_origin(statement: Node<'_>, source: &[u8]) -> ThrowOrigin {
    named_children(statement)
        .into_iter()
        .find(|child| child.kind() == CREATION)
        .and_then(|creation| creation.child_by_field_name(TYPE_FIELD))
        .map_or(ThrowOrigin::Unknown, |written| {
            ThrowOrigin::Thrown(written_type(text(written, source)))
        })
}

fn close_origin(resource: Node<'_>, source: &[u8], variables: &Variables) -> ThrowOrigin {
    if let Some(written) = resource.child_by_field_name(TYPE_FIELD) {
        let declared = written_type(text(written, source));
        return if declared == INFERRED {
            ThrowOrigin::Unknown
        } else {
            ThrowOrigin::Close(declared)
        };
    }
    named_children(resource)
        .first()
        .and_then(|variable| root_of(*variable, source, variables))
        .map_or(ThrowOrigin::Unknown, |root| match root {
            CallRoot::Type(declared) => ThrowOrigin::Close(declared),
            CallRoot::Scope(_) => ThrowOrigin::Unknown,
        })
}

// Las llamadas y referencias a métodos que quedan adentro de algún `try` de su clase: IntelliJ no
// marca un `throws` que un llamador atrapa con un `catch` de ese tipo.
pub fn caught_calls(nodes: &[Node<'_>], source: &[u8]) -> Vec<CaughtCall> {
    nodes
        .iter()
        .filter_map(|node| {
            let name = match node.kind() {
                INVOCATION => node.child_by_field_name(NAME_FIELD)?,
                METHOD_REFERENCE => *named_children(*node).last()?,
                _ => return None,
            };
            let caught = surrounding_catches(*node, source);
            (!caught.is_empty()).then(|| CaughtCall {
                name: text(name, source).to_owned(),
                caught,
            })
        })
        .collect()
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
