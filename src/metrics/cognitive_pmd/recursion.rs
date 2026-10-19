use tree_sitter::Node;

use crate::syntax::{descendants, named_children};

const METHOD: &str = "method_declaration";
const NAMED_TYPES: &[&str] = &[
    "class_declaration",
    "enum_declaration",
    "record_declaration",
    "interface_declaration",
];
const CLASS: &str = "class_declaration";
const ANONYMOUS_OWNER: &str = "object_creation_expression";
const TYPE_IDENTIFIER: &str = "type_identifier";
const GENERIC_TYPE: &str = "generic_type";
const IDENTIFIER: &str = "identifier";
const PARAMETER: &str = "formal_parameter";
const LOCAL_VARIABLE: &str = "local_variable_declaration";
const FIELD: &str = "field_declaration";
const DECLARATOR: &str = "variable_declarator";
const MAX_INHERITANCE_DEPTH: usize = 8;
const COMMENT_SUFFIX: &str = "comment";

struct Target<'source> {
    name: &'source str,
    arity: usize,
    own: bool,
    receivers: Vec<&'source str>,
    factories: Vec<&'source str>,
}

pub struct Recursion<'source> {
    targets: Vec<Target<'source>>,
}

pub struct Call<'source> {
    pub name: &'source str,
    pub arity: usize,
    pub receiver: Receiver<'source>,
}

pub enum Receiver<'source> {
    Implicit,
    This,
    Named(&'source str),
    Returned(&'source str),
    Other,
}

impl<'source> Recursion<'source> {
    pub fn of(unit: Node<'_>, source: &'source [u8]) -> Self {
        let root = std::iter::successors(Some(unit), Node::parent)
            .last()
            .unwrap_or(unit);
        let targets = std::iter::successors(Some(unit), Node::parent)
            .filter(|node| node.kind() == METHOD)
            .filter(|&method| same_signature_count(method, root, source) == 1)
            .filter_map(|method| {
                let owner = owner_name(method, source);
                Some(Target {
                    name: name_of(method, source)?,
                    arity: arity_of(method),
                    own: method.id() == unit.id(),
                    receivers: owner
                        .map(|owner| typed_receivers(method, owner, source))
                        .unwrap_or_default(),
                    factories: owner
                        .map(|owner| factories(method, owner, source))
                        .unwrap_or_default(),
                })
            })
            .collect();
        Self { targets }
    }

    pub fn matches(&self, call: &Call<'_>) -> bool {
        self.targets.iter().any(|target| {
            target.name == call.name
                && target.arity == call.arity
                && match call.receiver {
                    Receiver::Implicit => true,
                    Receiver::This => target.own,
                    Receiver::Named(name) => target.receivers.contains(&name),
                    Receiver::Returned(name) => target.factories.contains(&name),
                    Receiver::Other => false,
                }
        })
    }
}

fn name_of<'source>(node: Node<'_>, source: &'source [u8]) -> Option<&'source str> {
    node.child_by_field_name("name")?.utf8_text(source).ok()
}

fn arity_of(method: Node<'_>) -> usize {
    method.child_by_field_name("parameters").map_or(0, item_count)
}

pub fn item_count(list: Node<'_>) -> usize {
    named_children(list)
        .into_iter()
        .filter(|item| !item.kind().ends_with(COMMENT_SUFFIX))
        .count()
}

fn owner_name<'source>(method: Node<'_>, source: &'source [u8]) -> Option<&'source str> {
    method
        .parent()?
        .parent()
        .filter(|owner| NAMED_TYPES.contains(&owner.kind()))
        .and_then(|owner| name_of(owner, source))
}

fn factories<'source>(method: Node<'_>, owner: &str, source: &'source [u8]) -> Vec<&'source str> {
    method
        .parent()
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .filter(|member| member.kind() == METHOD)
        .filter(|member| {
            member
                .child_by_field_name("type")
                .and_then(|returned| simple_type_name(returned, source))
                == Some(owner)
        })
        .filter_map(|member| name_of(member, source))
        .collect()
}

fn same_signature_count(method: Node<'_>, root: Node<'_>, source: &[u8]) -> usize {
    let name = name_of(method, source);
    let arity = arity_of(method);
    let Some(body) = method.parent() else {
        return 0;
    };
    inherited_bodies(body, root, source)
        .into_iter()
        .flat_map(named_children)
        .filter(|sibling| sibling.kind() == METHOD)
        .filter(|&sibling| name_of(sibling, source) == name && arity_of(sibling) == arity)
        .count()
}

fn inherited_bodies<'tree>(body: Node<'tree>, root: Node<'tree>, source: &[u8]) -> Vec<Node<'tree>> {
    let mut bodies = vec![body];
    let mut current = body;
    while bodies.len() < MAX_INHERITANCE_DEPTH {
        let Some(parent_name) = current.parent().and_then(|owner| supertype_name(owner, source)) else {
            break;
        };
        let parent_body = descendants(root)
            .into_iter()
            .find(|class| class.kind() == CLASS && name_of(*class, source) == Some(parent_name))
            .and_then(|class| class.child_by_field_name("body"));
        let Some(parent_body) =
            parent_body.filter(|found| !bodies.iter().any(|seen| seen.id() == found.id()))
        else {
            break;
        };
        bodies.push(parent_body);
        current = parent_body;
    }
    bodies
}

fn supertype_name<'source>(owner: Node<'_>, source: &'source [u8]) -> Option<&'source str> {
    let supertype = match owner.kind() {
        CLASS => owner
            .child_by_field_name("superclass")
            .and_then(|superclass| named_children(superclass).into_iter().next()),
        ANONYMOUS_OWNER => owner.child_by_field_name("type"),
        _ => None,
    }?;
    simple_type_name(supertype, source)
}

fn simple_type_name<'source>(type_node: Node<'_>, source: &'source [u8]) -> Option<&'source str> {
    match type_node.kind() {
        TYPE_IDENTIFIER => type_node.utf8_text(source).ok(),
        GENERIC_TYPE => named_children(type_node)
            .into_iter()
            .next()
            .and_then(|base| simple_type_name(base, source)),
        _ => None,
    }
}

fn typed_receivers<'source>(method: Node<'_>, owner: &str, source: &'source [u8]) -> Vec<&'source str> {
    let fields = method
        .parent()
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .filter(|member| member.kind() == FIELD);
    descendants(method)
        .into_iter()
        .filter(|node| [PARAMETER, LOCAL_VARIABLE].contains(&node.kind()))
        .chain(fields)
        .filter(|declaration| {
            declaration
                .child_by_field_name("type")
                .and_then(|declared| simple_type_name(declared, source))
                == Some(owner)
        })
        .flat_map(|declaration| declared_names(declaration, source))
        .collect()
}

fn declared_names<'source>(declaration: Node<'_>, source: &'source [u8]) -> Vec<&'source str> {
    if declaration.kind() == PARAMETER {
        return name_of(declaration, source).into_iter().collect();
    }
    named_children(declaration)
        .into_iter()
        .filter(|child| child.kind() == DECLARATOR)
        .filter_map(|declarator| declarator.child_by_field_name("name"))
        .filter(|name| name.kind() == IDENTIFIER)
        .filter_map(|name| name.utf8_text(source).ok())
        .collect()
}
