use std::rc::Rc;

use tree_sitter::Node;

use super::java::{CONSTRUCTOR, METHOD, NAME_FIELD, text};
use super::java_call_roots::{root_of, with_arguments};
use super::java_calls::TYPE_DECLARATIONS;
use super::java_chains::argument_count;
use super::java_constructor_throws::{NO_ARGUMENTS, constructor_points, explicit_origin};
use super::java_functional_arguments::argument_kinds;
use super::java_receivers::{Variables, enclosing_types, simple_type_name};
use crate::syntax::{descendants, named_children, one_based};

const THROWS: &str = "throws";
const INVOCATION: &str = "method_invocation";
const CREATION: &str = "object_creation_expression";
const EXPLICIT_CONSTRUCTOR: &str = "explicit_constructor_invocation";
const THROW: &str = "throw_statement";
const RESOURCE: &str = "resource";
pub(super) const LAMBDA: &str = "lambda_expression";
pub(super) const METHOD_REFERENCE: &str = "method_reference";
pub const CONSTRUCTOR_REFERENCE: &str = "new";
const CLASS_BODY: &str = "class_body";
const TRY_STATEMENTS: &[&str] = &["try_statement", "try_with_resources_statement"];
const CATCH_CLAUSE: &str = "catch_clause";
const CATCH_PARAMETER: &str = "catch_formal_parameter";
const CATCH_TYPE: &str = "catch_type";
const FINALLY: &str = "finally_clause";
const IDENTIFIER: &str = "identifier";
const INFERRED: &str = "var";
pub(super) const BODY_FIELD: &str = "body";
const TYPE_FIELD: &str = "type";
const OBJECT_FIELD: &str = "object";
const GENERIC_OPEN: char = '<';
const ARRAY_OPEN: char = '[';

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
    Generic(String, Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallStep {
    pub name: String,
    pub arity: usize,
    pub arguments: Vec<ArgumentKind>,
}

// El tipo de un argumento cuando se ve en el propio texto de la llamada: un literal de texto o una
// codificación de caracteres sirven para elegir entre sobrecargas como `getBytes(String)` y
// `getBytes(Charset)`; una lambda (con lo que tira su cuerpo) o una referencia a método, para saber
// qué es el `E` de un `throws E` genérico. Lo demás no se sabe sin compilar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgumentKind {
    Unknown,
    Text,
    Charset,
    Lambda(Vec<ThrowPoint>),
    Reference(CallRoot, String),
}

// Lo que en un cuerpo puede tirar una excepción chequeada. `Unknown` es lo que no se sabe sin
// compilar: como IntelliJ con una llamada sin resolver, alcanza para no opinar sobre el método.
// `Rethrown` es un `throw` de una variable, que puede llevar cualquier subtipo del suyo.
// `Anonymous` es un `new X() { }` sin argumentos: no tira nada, pero IntelliJ no le resuelve el
// constructor si `X` es una interfaz, y entonces no opina sobre el método.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThrowOrigin {
    Unknown,
    Thrown(String),
    Rethrown(String),
    Creation(String, usize),
    Close(String),
    Call(CallRoot, Vec<CallStep>),
    Anonymous(String),
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

pub fn method_throws(method: Node<'_>, source: &[u8], variables: &Variables) -> Option<MethodThrows> {
    let clause_list = named_children(method)
        .into_iter()
        .find(|child| child.kind() == THROWS)?;
    let name = method.child_by_field_name(NAME_FIELD)?.start_position();
    let points = method
        .child_by_field_name(BODY_FIELD)
        .map(|body| {
            let scoped = with_catch_parameters(body, source, variables);
            let mut points = match method.kind() {
                METHOD => throw_points(body, source, &scoped),
                CONSTRUCTOR => constructor_points(method, body, source, &scoped),
                _ => Vec::new(),
            };
            points.extend(anonymous_points(body, source));
            points
        })
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

pub(super) fn written_type(written: &str) -> String {
    let without_arguments = written.split(GENERIC_OPEN).next().unwrap_or(written);
    without_arguments
        .split(ARRAY_OPEN)
        .next()
        .unwrap_or(without_arguments)
        .trim()
        .to_owned()
}

// Una lambda o una clase anónima no tiran al declararse: lo de adentro corre en otro método.
pub(super) fn throw_points(body: Node<'_>, source: &[u8], variables: &Variables) -> Vec<ThrowPoint> {
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

// IntelliJ junta lo que tira un método recorriendo también las clases anónimas y locales de
// adentro, pero no las lambdas.
fn anonymous_points(body: Node<'_>, source: &[u8]) -> Vec<ThrowPoint> {
    let mut pending = vec![body];
    let mut points = Vec::new();
    while let Some(node) = pending.pop() {
        if node.kind() == LAMBDA {
            continue;
        }
        if let Some(base) = anonymous_base(node, source) {
            points.push(ThrowPoint {
                origin: ThrowOrigin::Anonymous(base),
                caught: Vec::new(),
            });
        }
        pending.extend(named_children(node));
    }
    points
}

fn anonymous_base(node: Node<'_>, source: &[u8]) -> Option<String> {
    let is_anonymous = node.kind() == CREATION
        && argument_count(node) == NO_ARGUMENTS
        && named_children(node)
            .iter()
            .any(|child| child.kind() == CLASS_BODY);
    if !is_anonymous {
        return None;
    }
    node.child_by_field_name(TYPE_FIELD)
        .map(|written| written_type(text(written, source)))
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

pub(super) fn caught_types(clause: Node<'_>, source: &[u8]) -> Vec<String> {
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
        THROW => Some(thrown_origin(node, source, variables)),
        RESOURCE => Some(close_origin(node, source, variables)),
        EXPLICIT_CONSTRUCTOR => explicit_origin(node, source),
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

pub(super) fn call_origin(invocation: Node<'_>, source: &[u8], variables: &Variables) -> ThrowOrigin {
    let mut steps = Vec::new();
    let mut current = invocation;
    loop {
        let Some(name) = current.child_by_field_name(NAME_FIELD) else {
            return ThrowOrigin::Unknown;
        };
        steps.push(CallStep {
            name: text(name, source).to_owned(),
            arity: argument_count(current),
            arguments: argument_kinds(current, source, variables),
        });
        let Some(object) = current.child_by_field_name(OBJECT_FIELD) else {
            steps.reverse();
            return ThrowOrigin::Call(CallRoot::Scope(enclosing_types(current, source)), steps);
        };
        if object.kind() != INVOCATION {
            steps.reverse();
            return root_of(object, source, variables).map_or(ThrowOrigin::Unknown, |root| {
                ThrowOrigin::Call(with_arguments(root, object, source, variables), steps)
            });
        }
        current = object;
    }
}

fn creation_origin(creation: Node<'_>, source: &[u8]) -> ThrowOrigin {
    creation
        .child_by_field_name(TYPE_FIELD)
        .map_or(ThrowOrigin::Unknown, |written| {
            ThrowOrigin::Creation(written_type(text(written, source)), argument_count(creation))
        })
}

// `throw new X()` tira un `X`. `throw e` puede ser el relanzamiento preciso de un `catch`, que pide
// flujo de datos para saber qué lleva: se toma el tipo declarado de la variable como cota.
fn thrown_origin(statement: Node<'_>, source: &[u8], variables: &Variables) -> ThrowOrigin {
    let Some(thrown) = named_children(statement).into_iter().next() else {
        return ThrowOrigin::Unknown;
    };
    match thrown.kind() {
        CREATION => thrown
            .child_by_field_name(TYPE_FIELD)
            .map_or(ThrowOrigin::Unknown, |written| {
                ThrowOrigin::Thrown(written_type(text(written, source)))
            }),
        IDENTIFIER => match variables.get(text(thrown, source)) {
            Some(Some(declared)) => ThrowOrigin::Rethrown(declared.clone()),
            Some(None) | None => ThrowOrigin::Unknown,
        },
        _ => ThrowOrigin::Unknown,
    }
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
            CallRoot::Type(declared) | CallRoot::Generic(declared, _) => ThrowOrigin::Close(declared),
            CallRoot::Scope(_) => ThrowOrigin::Unknown,
        })
}
