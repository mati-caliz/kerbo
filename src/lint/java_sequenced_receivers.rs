use tree_sitter::Node;

use super::java::{
    BODY_FIELD, IDENTIFIER, METHOD, NAME_FIELD, TYPE_FIELD, VARIABLE_DECLARATOR, is_type_declaration, text,
};
use super::java_receivers::declared_type;
use crate::syntax::{descendants, named_children};

const INVOCATION: &str = "method_invocation";
const FIELD_ACCESS: &str = "field_access";
const FIELD_DECLARATION: &str = "field_declaration";
const THIS: &str = "this";
const OBJECT_FIELD: &str = "object";
const FIELD_FIELD: &str = "field";
const TYPE_ARGUMENTS_START: char = '<';
const NAME_SEPARATOR: char = '.';

// El tipo escrito del receptor: una variable o un campo propio, el campo de una variable cuya clase
// se declara en el mismo archivo, o lo que devuelve un método de las clases que encierran la
// llamada (si todas sus declaraciones con ese nombre devuelven lo mismo).
pub(super) fn receiver_type<'tree>(receiver: Node<'tree>, source: &[u8]) -> Option<Node<'tree>> {
    match receiver.kind() {
        IDENTIFIER => declared_type(receiver, text(receiver, source), source),
        FIELD_ACCESS => {
            let object = receiver.child_by_field_name(OBJECT_FIELD)?;
            let field = receiver.child_by_field_name(FIELD_FIELD)?;
            if object.kind() == THIS {
                return declared_type(field, text(field, source), source);
            }
            let owner = receiver_type(object, source)?;
            let owner_name = simple_name(text(owner, source));
            let declaration = type_declared_in_file(receiver, &owner_name, source)?;
            field_type(declaration, text(field, source), source)
        }
        INVOCATION if receiver.child_by_field_name(OBJECT_FIELD).is_none() => {
            let name = text(receiver.child_by_field_name(NAME_FIELD)?, source);
            own_method_type(receiver, name, source)
        }
        _ => None,
    }
}

pub(super) fn simple_name(written: &str) -> String {
    let raw = written.split(TYPE_ARGUMENTS_START).next().unwrap_or_default();
    raw.rsplit(NAME_SEPARATOR).next().unwrap_or(raw).trim().to_owned()
}

fn type_declared_in_file<'tree>(usage: Node<'tree>, name: &str, source: &[u8]) -> Option<Node<'tree>> {
    let root = std::iter::successors(Some(usage), Node::parent).last()?;
    descendants(root).into_iter().find(|declaration| {
        is_type_declaration(*declaration)
            && declaration
                .child_by_field_name(NAME_FIELD)
                .is_some_and(|declared| text(declared, source) == name)
    })
}

fn field_type<'tree>(declaration: Node<'tree>, name: &str, source: &[u8]) -> Option<Node<'tree>> {
    let body = declaration.child_by_field_name(BODY_FIELD)?;
    named_children(body)
        .into_iter()
        .filter(|member| member.kind() == FIELD_DECLARATION)
        .find(|member| {
            named_children(*member).into_iter().any(|child| {
                child.kind() == VARIABLE_DECLARATOR
                    && child
                        .child_by_field_name(NAME_FIELD)
                        .is_some_and(|declared| text(declared, source) == name)
            })
        })
        .and_then(|member| member.child_by_field_name(TYPE_FIELD))
}

fn own_method_type<'tree>(call: Node<'tree>, name: &str, source: &[u8]) -> Option<Node<'tree>> {
    std::iter::successors(call.parent(), Node::parent)
        .filter(|ancestor| is_type_declaration(*ancestor))
        .find_map(|declaration| {
            let returned: Vec<Node<'tree>> = declaration
                .child_by_field_name(BODY_FIELD)
                .map(named_children)
                .unwrap_or_default()
                .into_iter()
                .filter(|member| {
                    member.kind() == METHOD
                        && member
                            .child_by_field_name(NAME_FIELD)
                            .is_some_and(|declared| text(declared, source) == name)
                })
                .filter_map(|method| method.child_by_field_name(TYPE_FIELD))
                .collect();
            let first = *returned.first()?;
            returned
                .iter()
                .all(|other| text(*other, source) == text(first, source))
                .then_some(first)
        })
}

pub(super) fn inside_type_named(node: Node<'_>, name: &str, source: &[u8]) -> bool {
    std::iter::successors(node.parent(), Node::parent).any(|ancestor| {
        is_type_declaration(ancestor)
            && ancestor
                .child_by_field_name(NAME_FIELD)
                .is_some_and(|declared| text(declared, source) == name)
    })
}
