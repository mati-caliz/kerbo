use tree_sitter::Node;

use super::java::text;
use super::java_owner::{NullMarking, null_marking_of};
use super::java_receivers::{declared_type, enclosing_types};
use super::java_type_variables::type_parameter_admits_null;
use crate::syntax::named_children;

const INVOCATION: &str = "method_invocation";
const OBJECT_FIELD: &str = "object";
const IDENTIFIER: &str = "identifier";
const TYPE_ARGUMENTS: &str = "type_arguments";
const ANNOTATED_TYPE: &str = "annotated_type";
const WILDCARD: &str = "wildcard";
const TYPE_IDENTIFIER: &str = "type_identifier";
const NULLABLE: &str = "Nullable";
const METHOD: &str = "method_declaration";
const NAME_FIELD: &str = "name";
const TYPE_FIELD: &str = "type";
const ARGUMENTS_FIELD: &str = "arguments";
const PARAMETERS_FIELD: &str = "parameters";
const MEMBER_BODIES: &[&str] = &["class_body", "interface_body", "enum_body_declarations"];
const COMMENTS: &[&str] = &["line_comment", "block_comment"];

// Los argumentos de tipo con que se declara el receptor de una llamada (`Multiset<String> ms` en
// `ms.setCount(null, 1, 0)`) y lo que dice de nulos el código que llama. Bajo `@NullMarked`, un
// argumento concreto sin anotar no admite null, e IntelliJ lo pone en lugar de la variable de tipo
// del parámetro.
// Si el receptor es un campo o un getter que el archivo no declara (`collection` o `getMap()`
// heredados), las clases que encierran la llamada, donde el índice busca de quién lo heredan.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReceiverArguments {
    pub marking: NullMarking,
    pub arguments: Vec<ArgumentNullness>,
    pub inherited: Option<InheritedMember>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InheritedMember {
    pub scope: Vec<String>,
    pub name: String,
    pub getter: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgumentNullness {
    Concrete,
    Nullable,
    NonNullVariable,
    Unknown,
}

pub fn receiver_arguments(call: Node<'_>, source: &[u8]) -> ReceiverArguments {
    let Some(object) = call
        .child_by_field_name(OBJECT_FIELD)
        .filter(|_| call.kind() == INVOCATION)
    else {
        return ReceiverArguments::default();
    };
    let declared = match object.kind() {
        IDENTIFIER => declared_type(object, text(object, source), source),
        INVOCATION => getter_type(object, source),
        _ => None,
    };
    let Some(written) = declared else {
        return ReceiverArguments {
            inherited: inherited_member(object, source),
            ..ReceiverArguments::default()
        };
    };
    let arguments = arguments_of(call, written, source);
    if arguments.is_empty() {
        return ReceiverArguments::default();
    }
    ReceiverArguments {
        marking: null_marking_of(call, source),
        arguments,
        inherited: None,
    }
}

// Un campo en minúscula o un getter sin receptor ni argumentos que el archivo no declara.
fn inherited_member(object: Node<'_>, source: &[u8]) -> Option<InheritedMember> {
    let (name, getter) = match object.kind() {
        IDENTIFIER => (text(object, source), false),
        INVOCATION if is_unqualified_without_arguments(object) => {
            (text(object.child_by_field_name(NAME_FIELD)?, source), true)
        }
        _ => return None,
    };
    if !name.starts_with(|first: char| first.is_ascii_lowercase()) {
        return None;
    }
    Some(InheritedMember {
        scope: enclosing_types(object, source),
        name: name.to_owned(),
        getter,
    })
}

fn is_unqualified_without_arguments(invocation: Node<'_>) -> bool {
    invocation.kind() == INVOCATION
        && invocation.child_by_field_name(OBJECT_FIELD).is_none()
        && !invocation
            .child_by_field_name(ARGUMENTS_FIELD)
            .is_some_and(has_named_children)
}

fn arguments_of(call: Node<'_>, written: Node<'_>, source: &[u8]) -> Vec<ArgumentNullness> {
    named_children(written)
        .into_iter()
        .filter(|child| child.kind() == TYPE_ARGUMENTS)
        .flat_map(named_children)
        .map(|argument| match argument.kind() {
            ANNOTATED_TYPE if text(argument, source).contains(NULLABLE) => ArgumentNullness::Nullable,
            ANNOTATED_TYPE | WILDCARD => ArgumentNullness::Unknown,
            TYPE_IDENTIFIER => match type_parameter_admits_null(call, text(argument, source), source) {
                Some(false) => ArgumentNullness::NonNullVariable,
                Some(true) => ArgumentNullness::Unknown,
                None => ArgumentNullness::Concrete,
            },
            _ => ArgumentNullness::Concrete,
        })
        .collect()
}

// El tipo que declara un getter de las clases que encierran la llamada (`getMap()` sin receptor ni
// argumentos), si el archivo lo declara.
pub fn getter_type<'tree>(invocation: Node<'tree>, source: &[u8]) -> Option<Node<'tree>> {
    if !is_unqualified_without_arguments(invocation) {
        return None;
    }
    let name = text(invocation.child_by_field_name(NAME_FIELD)?, source);
    std::iter::successors(invocation.parent(), Node::parent)
        .filter(|node| MEMBER_BODIES.contains(&node.kind()))
        .find_map(|body| {
            named_children(body).into_iter().find(|member| {
                member.kind() == METHOD
                    && member
                        .child_by_field_name(NAME_FIELD)
                        .is_some_and(|declared| text(declared, source) == name)
                    && member
                        .child_by_field_name(PARAMETERS_FIELD)
                        .is_some_and(|parameters| !has_named_children(parameters))
            })
        })
        .and_then(|getter| getter.child_by_field_name(TYPE_FIELD))
}

fn has_named_children(node: Node<'_>) -> bool {
    named_children(node)
        .iter()
        .any(|child| !COMMENTS.contains(&child.kind()))
}
