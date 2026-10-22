use std::collections::HashSet;

use tree_sitter::Node;

use super::java::{CONSTRUCTOR, METHOD, NAME_FIELD, PRIVATE, children, modifiers, text};
use super::java_chains::{Chain, NullFacts, chain_of};
use super::java_constants::{Constants, constant_value, constants_in};
use super::java_null_flow::{NullFlow, null_flows};
use super::java_owner::{Owner, annotation_names, mentioned_name, owner_of};
use super::java_receivers::{
    Receiver, StaticImports, Variables, simple_type_name, supertypes, variable_types,
};
use super::java_resources::{ResourceCandidate, resource_candidates};
pub use super::java_returns::{ReturnType, Returns};
use super::java_returns::{return_type, returned};
use super::java_switch::{rule_bodies, yields_value};
use super::java_taint::taint_method;
use super::taint::TaintMethod;
use crate::syntax::{descendants, named_children};

pub const TYPE_DECLARATIONS: &[&str] = &[
    "class_declaration",
    "enum_declaration",
    "record_declaration",
    "interface_declaration",
    "annotation_type_declaration",
];
const ENUM: &str = "enum_declaration";
const RECORD: &str = "record_declaration";
const PUBLIC_BODIES: &[&str] = &["interface_body", "annotation_type_body"];
const ENUM_CONSTANT: &str = "enum_constant";
const PUBLIC: &str = "public";
const PROTECTED: &str = "protected";
const OVERRIDE: &str = "Override";
const ANNOTATIONS: &[&str] = &["marker_annotation", "annotation"];
const INVOCATION: &str = "method_invocation";
const CREATION: &str = "object_creation_expression";
const EXPLICIT_CONSTRUCTOR: &str = "explicit_constructor_invocation";
const METHOD_REFERENCE: &str = "method_reference";
const EXPRESSION_STATEMENT: &str = "expression_statement";
const SWITCH: &str = "switch_expression";
const STRING_LITERAL: &str = "string_literal";
const ARGUMENTS_FIELD: &str = "arguments";
const PARAMETERS_FIELD: &str = "parameters";
const BODY_FIELD: &str = "body";
const TYPE_FIELD: &str = "type";
const FORMAL_PARAMETER: &str = "formal_parameter";
const SPREAD_PARAMETER: &str = "spread_parameter";
const VARIABLE_DECLARATOR: &str = "variable_declarator";
const THIS: &str = "this";
const COMMENTS: &[&str] = &["line_comment", "block_comment"];
const QUALIFIER_SEPARATOR: char = '.';

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Private,
    Package,
    Protected,
    Public,
}

// Si el método tiene cuerpo escrito o es abstracto (o de una interfaz, sin `default`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Body {
    Written,
    Abstract,
}

// Si la cantidad de parámetros es fija o termina en varargs (`String... valores`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arity {
    Fixed,
    Variadic,
}

// Un método o constructor: el nombre (el del tipo para un constructor), sus parámetros y lo que
// importa para saber si se puede razonar sobre él sin compilar.
pub struct Callable<'tree> {
    pub name: String,
    pub name_node: Node<'tree>,
    pub parameters: Vec<Node<'tree>>,
    pub is_constructor: bool,
    pub visibility: Visibility,
    pub overrides: bool,
    pub body: Body,
    pub arity: Arity,
    pub returns: Returns,
    pub return_type: ReturnType,
    pub returned: String,
    pub annotations: Vec<String>,
    pub owner: Owner,
}

// Una llamada: el nombre, por cada argumento el valor constante como lo muestra IntelliJ (o nada
// si no es constante), y si el resultado se usa.
pub struct Call {
    pub name: String,
    pub is_constructor: bool,
    pub receiver: Receiver,
    pub arguments: Vec<Option<String>>,
    pub result_used: bool,
}

#[derive(Default)]
pub struct Extracted<'tree> {
    pub callables: Vec<Callable<'tree>>,
    pub calls: Vec<Call>,
    pub referenced: Vec<String>,
    pub types: Vec<String>,
    pub mentioned: Vec<String>,
    pub hierarchy: Vec<(String, Vec<String>)>,
    pub chains: Vec<Chain>,
    pub null_flows: Vec<NullFlow>,
    pub resources: Vec<ResourceCandidate>,
    pub taints: Vec<TaintMethod>,
}

// Lo que declara y lo que llama una clase de nivel superior, con sus clases anidadas.
pub fn extract<'tree>(top_level: Node<'tree>, source: &[u8]) -> Extracted<'tree> {
    let nodes = descendants(top_level);
    let constants = constants_in(&nodes, source);
    let mut discarded: HashSet<usize> = HashSet::new();
    let mut yielded: HashSet<usize> = HashSet::new();
    let mut extracted = Extracted::default();
    let mut facts = NullFacts::default();
    let file_variables = variable_types(&nodes, source);
    let mut scopes: Vec<(usize, Variables)> = Vec::new();
    let imported = StaticImports::of(top_level, source);
    for &node in &nodes {
        while scopes.last().is_some_and(|(end, _)| *end <= node.start_byte()) {
            scopes.pop();
        }
        if node.kind() == METHOD || node.kind() == CONSTRUCTOR {
            scopes.push((node.end_byte(), method_variables(node, source, &file_variables)));
        }
        let variables = scopes.last().map_or(&file_variables, |(_, scoped)| scoped);
        facts.observe(node, source);
        if TYPE_DECLARATIONS.contains(&node.kind())
            && let Some(name) = node.child_by_field_name(NAME_FIELD)
        {
            extracted.types.push(text(name, source).to_owned());
            extracted
                .hierarchy
                .push((text(name, source).to_owned(), supertypes(node, source)));
        }
        match node.kind() {
            METHOD | CONSTRUCTOR => {
                facts.enter_method();
                extracted.callables.extend(callable(node, source));
                if let Some(body) = node.child_by_field_name(BODY_FIELD) {
                    extracted.null_flows.extend(null_flows(body, source, variables));
                    extracted
                        .resources
                        .extend(resource_candidates(body, source, variables));
                }
                extracted
                    .taints
                    .extend(taint_method(node, source, variables, &imported));
            }
            ENUM => extracted
                .callables
                .extend(implicit_enum_constructor(node, source)),
            RECORD => extracted.callables.extend(record_constructor(node, source)),
            SWITCH if yields_value(node) => yielded.extend(rule_bodies(node)),
            EXPRESSION_STATEMENT if !yielded.contains(&node.id()) => {
                discarded.extend(named_children(node).first().map(Node::id));
            }
            INVOCATION | CREATION | EXPLICIT_CONSTRUCTOR | ENUM_CONSTANT => {
                if node.kind() == INVOCATION {
                    extracted
                        .chains
                        .extend(chain_of(node, source, variables, &mut facts));
                }
                let receiver = || imported.receiver(node, source, variables);
                extracted.calls.extend(
                    call(node, source, &constants, !discarded.contains(&node.id())).map(|found| Call {
                        receiver: if node.kind() == INVOCATION {
                            receiver()
                        } else {
                            Receiver::Unknown
                        },
                        ..found
                    }),
                );
            }
            STRING_LITERAL => extracted.mentioned.extend(mentioned_name(node, source)),
            METHOD_REFERENCE => extracted.referenced.extend(
                named_children(node)
                    .last()
                    .map(|name| text(*name, source).to_owned()),
            ),
            _ => {}
        }
    }
    extracted
}

// Los tipos de las variables vistos desde un método: lo que declara él manda sobre lo del resto
// del archivo (el mismo nombre puede ser de otro tipo en otro método).
fn method_variables(method: Node<'_>, source: &[u8], file_variables: &Variables) -> Variables {
    let mut scoped = file_variables.clone();
    scoped.extend(variable_types(&descendants(method), source));
    scoped
}

fn callable<'tree>(declaration: Node<'tree>, source: &[u8]) -> Option<Callable<'tree>> {
    let name_node = declaration.child_by_field_name(NAME_FIELD)?;
    let parameters = declaration.child_by_field_name(PARAMETERS_FIELD)?;
    let written = named_children(parameters);
    let is_constructor = declaration.kind() != METHOD;
    Some(Callable {
        name: text(name_node, source).to_owned(),
        name_node,
        parameters: written
            .iter()
            .filter_map(|parameter| parameter_name(*parameter))
            .collect(),
        is_constructor,
        visibility: visibility(declaration, source),
        overrides: annotated_with(declaration, OVERRIDE, source),
        body: if declaration.child_by_field_name(BODY_FIELD).is_some() {
            Body::Written
        } else {
            Body::Abstract
        },
        arity: if written
            .iter()
            .any(|parameter| parameter.kind() == SPREAD_PARAMETER)
        {
            Arity::Variadic
        } else {
            Arity::Fixed
        },
        returns: if is_constructor {
            Returns::Nothing
        } else {
            returned(declaration, source)
        },
        return_type: return_type(declaration),
        returned: declaration
            .child_by_field_name(TYPE_FIELD)
            .map(|written_type| simple_type_name(text(written_type, source)))
            .unwrap_or_default(),
        annotations: annotation_names(declaration, source),
        owner: owner_of(declaration, source),
    })
}

// El nombre de un parámetro, también el de un varargs (`String... args`), que va en su declarador.
fn parameter_name(parameter: Node<'_>) -> Option<Node<'_>> {
    match parameter.kind() {
        FORMAL_PARAMETER => parameter.child_by_field_name(NAME_FIELD),
        SPREAD_PARAMETER => named_children(parameter)
            .into_iter()
            .find(|child| child.kind() == VARIABLE_DECLARATOR)
            .and_then(|declarator| declarator.child_by_field_name(NAME_FIELD)),
        _ => None,
    }
}

// Un record trae un constructor con sus componentes, con la visibilidad del record.
fn record_constructor<'tree>(declaration: Node<'tree>, source: &[u8]) -> Option<Callable<'tree>> {
    let mut constructor = callable(declaration, source)?;
    constructor.is_constructor = true;
    constructor.returns = Returns::Nothing;
    Some(constructor)
}

// Un enum sin constructor escrito tiene uno implícito sin parámetros: no se marca nada, pero
// cuenta para saber si una constante con argumentos va a otro.
fn implicit_enum_constructor<'tree>(declaration: Node<'tree>, source: &[u8]) -> Option<Callable<'tree>> {
    let body = declaration.child_by_field_name(BODY_FIELD)?;
    let name_node = declaration.child_by_field_name(NAME_FIELD)?;
    let declares_constructor = descendants(body).iter().any(|node| node.kind() == CONSTRUCTOR);
    (!declares_constructor).then(|| Callable {
        name: text(name_node, source).to_owned(),
        name_node,
        parameters: Vec::new(),
        is_constructor: true,
        visibility: Visibility::Private,
        overrides: false,
        body: Body::Abstract,
        arity: Arity::Fixed,
        returns: Returns::Nothing,
        return_type: ReturnType::Void,
        returned: String::new(),
        annotations: Vec::new(),
        owner: owner_of(declaration, source),
    })
}

// Los constructores de un enum son privados aunque no lo digan, y lo que declara una interfaz es
// público si no dice privado.
pub fn visibility(declaration: Node<'_>, source: &[u8]) -> Visibility {
    if has_modifier(declaration, PRIVATE, source) {
        Visibility::Private
    } else if has_modifier(declaration, PUBLIC, source) {
        Visibility::Public
    } else if has_modifier(declaration, PROTECTED, source) {
        Visibility::Protected
    } else if declaration
        .parent()
        .is_some_and(|body| PUBLIC_BODIES.contains(&body.kind()))
    {
        Visibility::Public
    } else {
        Visibility::Package
    }
}

pub fn has_modifier(node: Node<'_>, modifier: &str, source: &[u8]) -> bool {
    let written = modifiers(node).is_some_and(|modifiers| {
        children(modifiers)
            .into_iter()
            .any(|child| text(child, source) == modifier)
    });
    written || (modifier == PRIVATE && node.kind() == CONSTRUCTOR && in_enum(node))
}

fn annotated_with(node: Node<'_>, simple_name: &str, source: &[u8]) -> bool {
    modifiers(node).is_some_and(|modifiers| {
        named_children(modifiers)
            .into_iter()
            .filter(|child| ANNOTATIONS.contains(&child.kind()))
            .filter_map(|annotation| annotation.child_by_field_name(NAME_FIELD))
            .any(|name| {
                let written = text(name, source);
                written.rsplit(QUALIFIER_SEPARATOR).next() == Some(simple_name)
            })
    })
}

fn in_enum(constructor: Node<'_>) -> bool {
    constructor
        .parent()
        .and_then(|body| body.parent())
        .and_then(|enum_body| enum_body.parent())
        .is_some_and(|declaration| declaration.kind() == ENUM)
}

fn call(node: Node<'_>, source: &[u8], constants: &Constants, result_used: bool) -> Option<Call> {
    let (name, is_constructor) = match node.kind() {
        INVOCATION => (
            text(node.child_by_field_name(NAME_FIELD)?, source).to_owned(),
            false,
        ),
        CREATION => (
            simple_type_name(text(node.child_by_field_name(TYPE_FIELD)?, source)),
            true,
        ),
        _ => (enclosing_type_name(node, source)?, true),
    };
    let arguments = node
        .child_by_field_name(ARGUMENTS_FIELD)
        .map(|arguments| {
            named_children(arguments)
                .into_iter()
                .filter(|argument| !COMMENTS.contains(&argument.kind()))
                .map(|argument| constant_value(argument, source, constants))
                .collect()
        })
        .unwrap_or_default();
    Some(Call {
        name,
        is_constructor,
        receiver: Receiver::Unknown,
        arguments,
        result_used,
    })
}

// `this(...)` y las constantes de un enum llaman a un constructor de la clase que los contiene;
// `super(...)` va a la clase de arriba y no cuenta.
fn enclosing_type_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    if node.kind() == EXPLICIT_CONSTRUCTOR
        && named_children(node)
            .first()
            .is_none_or(|first| first.kind() != THIS)
    {
        return None;
    }
    let mut current = node.parent();
    while let Some(candidate) = current {
        if TYPE_DECLARATIONS.contains(&candidate.kind()) {
            return candidate
                .child_by_field_name(NAME_FIELD)
                .map(|name| text(name, source).to_owned());
        }
        current = candidate.parent();
    }
    None
}
