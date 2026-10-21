use tree_sitter::Node;

use super::java::{
    CONSTRUCTOR, METHOD, NAME_FIELD, PRIVATE, PROTECTED, PUBLIC, STATIC, VARIABLE_DECLARATOR, text,
};
use super::java_design::has_modifier;
use super::{LintFinding, LintKind, finding};
use crate::rules::JavaRule;
use crate::syntax::{descendants, named_children};

const FIELD: &str = "field_declaration";
const INTERFACE_FIELD: &str = "constant_declaration";
const BODIES: &[&str] = &["class_body", "enum_body_declarations", "interface_body"];

pub fn reported(nodes: &[Node<'_>], source: &[u8]) -> Vec<LintFinding> {
    let fields = fields_in_order(nodes, source);
    nodes
        .iter()
        .filter(|node| BODIES.contains(&node.kind()))
        .flat_map(|body| declaration_order(*body, source, &fields))
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Section {
    StaticVariables,
    InstanceVariables,
    Constructors,
    Methods,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Access {
    Public,
    Protected,
    Package,
    Private,
}

fn access_of(node: Node<'_>, source: &[u8]) -> Access {
    if has_modifier(node, PUBLIC, source) {
        Access::Public
    } else if has_modifier(node, PROTECTED, source) {
        Access::Protected
    } else if has_modifier(node, PRIVATE, source) {
        Access::Private
    } else {
        Access::Package
    }
}

// Calca DeclarationOrder: variables static, de instancia, constructores y métodos, y dentro de las
// variables de mayor a menor visibilidad. Una variable fuera de lugar no cambia el estado.
// Los nombres de todos los campos del archivo con el byte donde termina su declaración, en orden.
fn fields_in_order<'source>(nodes: &[Node<'_>], source: &'source [u8]) -> Vec<(usize, &'source str)> {
    let mut fields: Vec<(usize, &str)> = nodes
        .iter()
        .copied()
        .filter(|node| node.kind() == FIELD || node.kind() == INTERFACE_FIELD)
        .flat_map(|field| {
            named_children(field)
                .into_iter()
                .filter(|child| child.kind() == VARIABLE_DECLARATOR)
                .filter_map(|declarator| declarator.child_by_field_name(NAME_FIELD))
                .map(move |name| (field.end_byte(), text(name, source)))
        })
        .collect();
    fields.sort_unstable_by_key(|(end, _)| *end);
    fields
}

fn declaration_order(body: Node<'_>, source: &[u8], fields: &[(usize, &str)]) -> Vec<LintFinding> {
    let earlier_fields = |field: Node<'_>| -> Vec<&str> {
        let count = fields.partition_point(|(end, _)| *end <= field.end_byte());
        fields
            .get(..count)
            .unwrap_or_default()
            .iter()
            .map(|(_, name)| *name)
            .collect()
    };
    let mut findings = Vec::new();
    let mut section = Section::StaticVariables;
    let mut access = Access::Public;
    for member in named_children(body) {
        match member.kind() {
            CONSTRUCTOR => {
                if section > Section::Constructors {
                    findings.push(finding(
                        LintKind::Java(JavaRule::DeclarationOrder),
                        member,
                        "",
                        None,
                    ));
                } else {
                    section = Section::Constructors;
                }
            }
            METHOD => section = Section::Methods,
            FIELD | INTERFACE_FIELD => {
                let valid = field_in_order(member, source, &mut section, &mut access);
                let field_access = access_of(member, source);
                let wrong_access = field_access < access;
                if !wrong_access {
                    access = field_access;
                }
                let reported_access = valid
                    && wrong_access
                    && !refers_to_earlier_field(member, source, &earlier_fields(member));
                if !valid || reported_access {
                    findings.push(finding(
                        LintKind::Java(JavaRule::DeclarationOrder),
                        member,
                        "",
                        None,
                    ));
                }
            }
            _ => {}
        }
    }
    findings
}

// Como isForwardReference de Checkstyle: si el valor de un campo usa uno declarado antes o a sí
// mismo (en cualquier clase del archivo), moverlo cambiaría el resultado y no se marca la
// visibilidad, aunque la visibilidad esperada tampoco cambia. Un campo de interfaz sin `static`
// cuenta como de instancia.
fn refers_to_earlier_field(field: Node<'_>, source: &[u8], earlier: &[&str]) -> bool {
    named_children(field)
        .into_iter()
        .filter(|child| child.kind() == VARIABLE_DECLARATOR)
        .filter_map(|declarator| declarator.child_by_field_name("value"))
        .flat_map(descendants)
        .filter(|node| node.kind() == "identifier")
        .any(|identifier| earlier.contains(&text(identifier, source)))
}

// Una variable static después de otra cosa, o una de instancia después de constructores o métodos,
// está fuera de lugar; la primera de instancia reinicia la visibilidad esperada.
fn field_in_order(field: Node<'_>, source: &[u8], section: &mut Section, access: &mut Access) -> bool {
    if has_modifier(field, STATIC, source) {
        return *section <= Section::StaticVariables;
    }
    if *section > Section::InstanceVariables {
        return false;
    }
    if *section == Section::StaticVariables {
        *section = Section::InstanceVariables;
        *access = Access::Public;
    }
    true
}
