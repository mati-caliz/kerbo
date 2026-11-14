use tree_sitter::Node;

use super::java::{CONSTRUCTOR, FINAL, PRIVATE, STATIC, has_modifier, is_type_declaration, text};
use super::java_chains::argument_count;
use super::java_exact_receivers::ExactReceivers;
use super::java_lambda_returns::{LambdaReturn, lambda_returns_in};
use super::java_nested_arguments::{NestedNull, enclosing_argument};
use super::java_receiver_arguments::{ReceiverArguments, receiver_arguments};
use super::java_receivers::{Receiver, StaticImports, Variables, simple_type_name, variable_types};
use crate::syntax::{named_children, one_based};

const BODY_FIELD: &str = "body";
const VALUE_FIELD: &str = "value";
const TYPE_FIELD: &str = "type";
const NAME_FIELD: &str = "name";
const ARGUMENTS_FIELD: &str = "arguments";
const CONSEQUENCE_FIELD: &str = "consequence";
const ALTERNATIVE_FIELD: &str = "alternative";
const RETURN: &str = "return_statement";
const IF: &str = "if_statement";
const VARIABLE_DECLARATOR: &str = "variable_declarator";
const ARRAY_TYPE: &str = "array_type";
pub const ARRAY_SUFFIX: &str = "[]";
const CONDITION_FIELD: &str = "condition";
const PARAMETERS_FIELD: &str = "parameters";
const PARAMETER_FIRST_CHECKS: [&str; 3] = ["==null", "!=null", "instanceof"];
const NULL_FIRST_CHECKS: [&str; 2] = ["null==", "null!="];
const NULL: &str = "null_literal";
const CAST: &str = "cast_expression";
const PARENTHESIZED: &str = "parenthesized_expression";
const TERNARY: &str = "ternary_expression";
const INVOCATION: &str = "method_invocation";
const CREATION: &str = "object_creation_expression";
const CLASS_BODY: &str = "class_body";
const RETURN_BOUNDARIES: &[&str] = &["lambda_expression", "class_body"];
const INTERFACE: &str = "interface_declaration";
const ENUM: &str = "enum_declaration";
const RECORD: &str = "record_declaration";
const COMMENTS: &[&str] = &["line_comment", "block_comment"];
const OBJECT_FIELD: &str = "object";
const IDENTIFIER: &str = "identifier";
const PRIMITIVE_TYPES: &[&str] = &["integral_type", "floating_point_type", "boolean_type"];
const DECLARATOR_SIBLINGS: &[&str] = &[
    "modifiers",
    VARIABLE_DECLARATOR,
    "annotation",
    "marker_annotation",
];

// Un `null` escrito como argumento (también `(Tipo) null`), con la llamada que lo recibe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NullArgument {
    pub name: String,
    pub is_constructor: bool,
    pub arity: usize,
    pub receiver: Receiver,
    pub index: usize,
    pub line: usize,
    pub column: usize,
    pub cast: Option<String>,
    pub exact: bool,
    pub receiver_arguments: ReceiverArguments,
}

// Si algún `return` del método devuelve `null`, directo o en una rama de un ternario: IntelliJ
// infiere de eso que el método es `@Nullable`. Si alguna condición pregunta por un parámetro
// (`texto == null`, `texto != null`, `objeto instanceof Cloneable`), lo que infiere es un contrato
// (`null -> null`), y con un contrato ya no da el método por `@Nullable`. Un ternario sólo arma
// contrato si es lo que se devuelve: el que arma una variable (`mapa == null ? new HashMap<>() : mapa`)
// no.
pub fn returns_null(method: Node<'_>, source: &[u8]) -> bool {
    let Some(body) = method.child_by_field_name(BODY_FIELD) else {
        return false;
    };
    let checks = parameter_checks(method, source);
    let mut returns_null = false;
    let mut pending = vec![body];
    while let Some(node) = pending.pop() {
        let may_be_a_contract = node.kind() == IF || (node.kind() == TERNARY && is_returned(node));
        if may_be_a_contract && checks_a_parameter(node, source, &checks) {
            return false;
        }
        if node.kind() == RETURN {
            returns_null |= named_children(node)
                .first()
                .is_some_and(|value| may_be_null(*value));
        }
        if !RETURN_BOUNDARIES.contains(&node.kind()) {
            pending.extend(named_children(node));
        }
    }
    returns_null
}

fn is_returned(expression: Node<'_>) -> bool {
    let mut current = expression;
    while let Some(parent) = current.parent() {
        match parent.kind() {
            RETURN => return true,
            TERNARY | PARENTHESIZED => current = parent,
            _ => return false,
        }
    }
    false
}

// `p == null`, `p != null`, sus vueltas y `p instanceof` de cada parámetro, sin espacios.
fn parameter_checks(method: Node<'_>, source: &[u8]) -> Vec<String> {
    method
        .child_by_field_name(PARAMETERS_FIELD)
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .filter_map(parameter_name)
        .flat_map(|name| {
            let name = text(name, source);
            PARAMETER_FIRST_CHECKS
                .iter()
                .map(move |check| format!("{name}{check}"))
                .chain(
                    NULL_FIRST_CHECKS
                        .iter()
                        .map(move |check| format!("{check}{name}")),
                )
                .collect::<Vec<_>>()
        })
        .collect()
}

fn checks_a_parameter(node: Node<'_>, source: &[u8], checks: &[String]) -> bool {
    node.child_by_field_name(CONDITION_FIELD)
        .is_some_and(|condition| {
            let written: String = text(condition, source).split_whitespace().collect();
            checks.iter().any(|check| written.contains(check.as_str()))
        })
}

fn may_be_null(value: Node<'_>) -> bool {
    match value.kind() {
        NULL => true,
        PARENTHESIZED => named_children(value)
            .first()
            .is_some_and(|inner| may_be_null(*inner)),
        TERNARY => [CONSEQUENCE_FIELD, ALTERNATIVE_FIELD]
            .iter()
            .filter_map(|field| value.child_by_field_name(field))
            .any(may_be_null),
        _ => false,
    }
}

// El nombre de un parámetro; el de un varargs (`String... textos`) está en su declarador.
pub fn parameter_name(parameter: Node<'_>) -> Option<Node<'_>> {
    parameter.child_by_field_name(NAME_FIELD).or_else(|| {
        named_children(parameter)
            .into_iter()
            .find(|child| child.kind() == VARIABLE_DECLARATOR)
            .and_then(|declarator| declarator.child_by_field_name(NAME_FIELD))
    })
}

// El tipo con que se castea el `null` (`(Number) null` da `Number`), por su nombre simple.
pub(super) fn cast_type(argument: Node<'_>, source: &[u8]) -> Option<String> {
    match argument.kind() {
        CAST => argument.child_by_field_name(TYPE_FIELD).map(|written| {
            let simple = simple_type_name(text(written, source));
            if written.kind() == ARRAY_TYPE {
                format!("{simple}{ARRAY_SUFFIX}")
            } else {
                simple
            }
        }),
        PARENTHESIZED => named_children(argument)
            .first()
            .and_then(|inner| cast_type(*inner, source)),
        _ => None,
    }
}

// El tipo escrito de un parámetro a partir de su nombre; el de un varargs (`String... textos`) va
// antes de su declarador.
pub(super) fn parameter_type(name: Node<'_>) -> Option<Node<'_>> {
    let parameter = name.parent()?;
    if parameter.kind() != VARIABLE_DECLARATOR {
        return parameter.child_by_field_name(TYPE_FIELD);
    }
    named_children(parameter.parent()?)
        .into_iter()
        .find(|child| !DECLARATOR_SIBLINGS.contains(&child.kind()))
}

pub fn parameter_type_name(name: Node<'_>, source: &[u8]) -> String {
    parameter_type(name)
        .map(|written| simple_type_name(text(written, source)))
        .unwrap_or_default()
}

fn is_variadic(name: Node<'_>) -> bool {
    name.parent()
        .is_some_and(|parameter| parameter.kind() == VARIABLE_DECLARATOR)
}

// Si el parámetro puede recibir un `null`: no, si es de un tipo primitivo (un varargs de
// primitivos es un arreglo y sí puede).
pub fn accepts_null(name: Node<'_>) -> bool {
    is_variadic(name) || parameter_type(name).is_none_or(|written| !PRIMITIVE_TYPES.contains(&written.kind()))
}

fn null_inside(argument: Node<'_>) -> Option<Node<'_>> {
    match argument.kind() {
        NULL => Some(argument),
        CAST => argument.child_by_field_name(VALUE_FIELD).and_then(null_inside),
        PARENTHESIZED => named_children(argument).first().copied().and_then(null_inside),
        _ => None,
    }
}

// Si alguien puede redefinir el método: IntelliJ sólo infiere nulos de los que no (un constructor,
// lo estático, privado o `final`, y lo de una clase `final`, anónima, un record o un enum sin
// cuerpos en sus constantes no se toca).
pub fn is_overridable(method: Node<'_>) -> bool {
    if method.kind() == CONSTRUCTOR
        || has_modifier(method, STATIC)
        || has_modifier(method, PRIVATE)
        || has_modifier(method, FINAL)
    {
        return false;
    }
    let Some(owner) = method.parent().and_then(|body| body.parent()) else {
        return true;
    };
    if owner.kind() == CREATION || owner.kind() == RECORD {
        return false;
    }
    if owner.kind() == INTERFACE || !is_type_declaration(owner) {
        return true;
    }
    if owner.kind() == ENUM {
        return enum_has_bodies(owner);
    }
    !has_modifier(owner, FINAL)
}

fn enum_has_bodies(declaration: Node<'_>) -> bool {
    declaration
        .child_by_field_name(BODY_FIELD)
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .flat_map(named_children)
        .any(|part| part.kind() == CLASS_BODY)
}

// Los `null` que pasan como argumento las llamadas del archivo y las lambdas que devuelven `null`.
// Lo que se junta de los argumentos de las llamadas del archivo.
pub struct CallArguments {
    pub null_arguments: Vec<NullArgument>,
    pub nested_nulls: Vec<NestedNull>,
    pub lambda_returns: Vec<LambdaReturn>,
}

pub fn null_arguments_in(top_level: Node<'_>, nodes: &[Node<'_>], source: &[u8]) -> CallArguments {
    let variables = variable_types(nodes, source);
    let imported = StaticImports::of(top_level, source);
    let exact = ExactReceivers::of(nodes, source);
    let context = CallContext {
        variables: &variables,
        imported: &imported,
        exactly_constructed: &exact,
    };
    let mut null_arguments = Vec::new();
    let mut nested_nulls = Vec::new();
    for call in nodes {
        let passed = nulls_passed_to(*call, source, &context);
        if passed.is_empty() {
            continue;
        }
        if let Some(outer) = enclosing_argument(*call, source, &context) {
            nested_nulls.extend(passed.iter().map(|inner| NestedNull {
                inner: inner.clone(),
                outer: outer.clone(),
            }));
        }
        null_arguments.extend(passed);
    }
    CallArguments {
        null_arguments,
        nested_nulls,
        lambda_returns: lambda_returns_in(nodes, source, &context),
    }
}

// Lo que hace falta del archivo para saber a quién llama una llamada.
pub(super) struct CallContext<'context> {
    pub(super) variables: &'context Variables,
    pub(super) imported: &'context StaticImports,
    pub(super) exactly_constructed: &'context ExactReceivers,
}

// A quién llama una llamada: el nombre del método (o de la clase, si construye), si es un
// constructor y el receptor.
pub(super) struct CallTarget {
    pub(super) name: String,
    pub(super) is_constructor: bool,
    pub(super) receiver: Receiver,
}

pub(super) fn call_target(
    call: Node<'_>,
    source: &[u8],
    imported: &StaticImports,
    variables: &Variables,
) -> Option<CallTarget> {
    match call.kind() {
        INVOCATION => Some(CallTarget {
            name: text(call.child_by_field_name(NAME_FIELD)?, source).to_owned(),
            is_constructor: false,
            receiver: imported.receiver_through_class(call, source, variables),
        }),
        CREATION
            if !named_children(call)
                .iter()
                .any(|child| child.kind() == CLASS_BODY) =>
        {
            let name = simple_type_name(text(call.child_by_field_name(TYPE_FIELD)?, source));
            Some(CallTarget {
                name: name.clone(),
                is_constructor: true,
                receiver: Receiver::Type(name),
            })
        }
        _ => None,
    }
}

// Los `null` que una llamada pasa como argumento.
fn nulls_passed_to(call: Node<'_>, source: &[u8], context: &CallContext<'_>) -> Vec<NullArgument> {
    call.child_by_field_name(ARGUMENTS_FIELD)
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .filter(|argument| !COMMENTS.contains(&argument.kind()))
        .enumerate()
        .filter_map(|(index, argument)| {
            let literal = null_inside(argument)?;
            let mut found = argument_of_call(call, (index, literal), source, context)?;
            found.cast = cast_type(argument, source);
            Some(found)
        })
        .collect()
}

// El argumento en la posición `index` de la llamada, ubicado en el nodo `at`: a quién llama y con qué
// receptor.
pub(super) fn argument_of_call(
    call: Node<'_>,
    (index, at): (usize, Node<'_>),
    source: &[u8],
    context: &CallContext<'_>,
) -> Option<NullArgument> {
    let CallTarget {
        name,
        is_constructor,
        receiver,
    } = call_target(call, source, context.imported, context.variables)?;
    let exact = call.kind() == INVOCATION
        && call
            .child_by_field_name(OBJECT_FIELD)
            .filter(|object| object.kind() == IDENTIFIER)
            .is_some_and(|object| context.exactly_constructed.contains(object, source));
    let start = at.start_position();
    Some(NullArgument {
        name,
        is_constructor,
        arity: argument_count(call),
        receiver,
        index,
        line: one_based(start.row),
        column: one_based(start.column),
        cast: None,
        exact,
        receiver_arguments: receiver_arguments(call, source),
    })
}
