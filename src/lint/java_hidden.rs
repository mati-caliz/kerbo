use std::collections::HashSet;

use tree_sitter::Node;

use super::java::{METHOD, NAME_FIELD, STATIC, VARIABLE_DECLARATOR, children, modifiers, text};
use super::{LintFinding, LintKind, finding};
use crate::rules::JavaRule;
use crate::syntax::named_children;

const FIELD: &str = "field_declaration";
const LOCAL: &str = "local_variable_declaration";
const FORMAL_PARAMETER: &str = "formal_parameter";
const SPREAD_PARAMETER: &str = "spread_parameter";
const CATCH_PARAMETER: &str = "catch_formal_parameter";
const ENHANCED_FOR: &str = "enhanced_for_statement";
const LAMBDA: &str = "lambda_expression";
const INFERRED_PARAMETERS: &str = "inferred_parameters";
const IDENTIFIER: &str = "identifier";
const PARAMETERS_FIELD: &str = "parameters";
const INSTANCEOF: &str = "instanceof_expression";
const CLASS_BODY: &str = "class_body";
const ENUM_BODY: &str = "enum_body";
const ENUM_BODY_DECLARATIONS: &str = "enum_body_declarations";
const INTERFACE_BODIES: &[&str] = &["interface_body", "annotation_type_body"];
const INTERFACE_FIELD: &str = "constant_declaration";
const FRAME_OWNERS: &[&str] = &[
    "class_declaration",
    "enum_declaration",
    "record_declaration",
    "object_creation_expression",
    "enum_constant",
    "interface_declaration",
    "annotation_type_declaration",
];
const STATIC_OWNERS: &[&str] = &[
    "interface_declaration",
    "annotation_type_declaration",
    "record_declaration",
];
const ANONYMOUS_OWNER: &str = "object_creation_expression";
const STATIC_INITIALIZER: &str = "static_initializer";

struct Frame {
    instance: HashSet<String>,
    statics: HashSet<String>,
    is_static: bool,
}

#[derive(Debug, Clone, Copy)]
struct Context {
    in_static: bool,
    anonymous_depth: usize,
}

enum Step<'tree> {
    Enter(Node<'tree>, Context),
    LeaveFrame,
}

// Calca HiddenField: cada clase (también enums, records, interfaces, constantes de enum y clases
// anónimas) es un frame con sus campos static y de instancia; un frame es static si lo dice el
// modificador o es una interfaz o un record (un enum no). Una variable local, un parámetro, el de un
// catch, el de un for o el de una lambda tapa un campo static de cualquier frame que la encierre, o
// uno de instancia de los frames hasta el primero static, salvo en un contexto static: lo decide
// el método (si no está en una clase anónima), el campo o el bloque static más cercano. Se recorre
// el árbol una vez, de arriba hacia abajo: subir al padre en tree-sitter es caro.
pub fn reported(root: Node<'_>, source: &[u8]) -> Vec<LintFinding> {
    let mut findings = Vec::new();
    let mut frames: Vec<Frame> = Vec::new();
    let mut pending = vec![Step::Enter(
        root,
        Context {
            in_static: false,
            anonymous_depth: 0,
        },
    )];
    while let Some(step) = pending.pop() {
        let (node, outer) = match step {
            Step::LeaveFrame => {
                frames.pop();
                continue;
            }
            Step::Enter(node, context) => (node, context),
        };
        for name in declared_names(node) {
            if hides_a_field(text(name, source), &frames, outer.in_static) {
                findings.push(finding(
                    LintKind::Java(JavaRule::HiddenField),
                    name,
                    text(name, source),
                    None,
                ));
            }
        }
        let inner = context_inside(node, outer, source);
        if FRAME_OWNERS.contains(&node.kind()) {
            frames.push(frame_of(node, source));
            pending.push(Step::LeaveFrame);
        }
        pending.extend(
            named_children(node)
                .into_iter()
                .rev()
                .map(|child| Step::Enter(child, inner)),
        );
    }
    findings
}

fn context_inside(node: Node<'_>, outer: Context, source: &[u8]) -> Context {
    match node.kind() {
        STATIC_INITIALIZER => Context {
            in_static: true,
            ..outer
        },
        METHOD if outer.anonymous_depth == 0 => Context {
            in_static: has_modifier(node, STATIC, source),
            ..outer
        },
        FIELD => Context {
            in_static: has_modifier(node, STATIC, source),
            ..outer
        },
        ANONYMOUS_OWNER => Context {
            anonymous_depth: outer.anonymous_depth.saturating_add(1),
            ..outer
        },
        _ => outer,
    }
}

fn declared_names(node: Node<'_>) -> Vec<Node<'_>> {
    match node.kind() {
        LOCAL | SPREAD_PARAMETER => named_children(node)
            .into_iter()
            .filter(|child| child.kind() == VARIABLE_DECLARATOR)
            .filter_map(|declarator| declarator.child_by_field_name(NAME_FIELD))
            .collect(),
        FORMAL_PARAMETER | CATCH_PARAMETER | ENHANCED_FOR | INSTANCEOF => {
            node.child_by_field_name(NAME_FIELD).into_iter().collect()
        }
        LAMBDA => lambda_names(node),
        _ => Vec::new(),
    }
}

fn lambda_names(lambda: Node<'_>) -> Vec<Node<'_>> {
    let Some(parameters) = lambda.child_by_field_name(PARAMETERS_FIELD) else {
        return Vec::new();
    };
    match parameters.kind() {
        IDENTIFIER => vec![parameters],
        INFERRED_PARAMETERS => named_children(parameters)
            .into_iter()
            .filter(|child| child.kind() == IDENTIFIER)
            .collect(),
        _ => Vec::new(),
    }
}

fn hides_a_field(name: &str, frames: &[Frame], in_static: bool) -> bool {
    if frames.iter().any(|frame| frame.statics.contains(name)) {
        return true;
    }
    if in_static {
        return false;
    }
    for frame in frames.iter().rev() {
        if frame.instance.contains(name) {
            return true;
        }
        if frame.is_static {
            return false;
        }
    }
    false
}

fn frame_of(owner: Node<'_>, source: &[u8]) -> Frame {
    let mut frame = Frame {
        instance: HashSet::new(),
        statics: HashSet::new(),
        is_static: STATIC_OWNERS.contains(&owner.kind()) || has_modifier(owner, STATIC, source),
    };
    for field in fields_of(owner) {
        let target = if has_modifier(field, STATIC, source) {
            &mut frame.statics
        } else {
            &mut frame.instance
        };
        target.extend(
            named_children(field)
                .into_iter()
                .filter(|child| child.kind() == VARIABLE_DECLARATOR)
                .filter_map(|declarator| declarator.child_by_field_name(NAME_FIELD))
                .map(|name| text(name, source).to_owned()),
        );
    }
    frame
}

fn fields_of(owner: Node<'_>) -> Vec<Node<'_>> {
    named_children(owner)
        .into_iter()
        .filter(|child| {
            child.kind() == CLASS_BODY
                || child.kind() == ENUM_BODY
                || INTERFACE_BODIES.contains(&child.kind())
        })
        .flat_map(|body| {
            let declarations: Vec<Node<'_>> = named_children(body)
                .into_iter()
                .filter(|child| child.kind() == ENUM_BODY_DECLARATIONS)
                .collect();
            std::iter::once(body).chain(declarations)
        })
        .flat_map(named_children)
        .filter(|member| member.kind() == FIELD || member.kind() == INTERFACE_FIELD)
        .collect()
}

fn has_modifier(node: Node<'_>, modifier: &str, source: &[u8]) -> bool {
    modifiers(node).is_some_and(|modifiers| {
        children(modifiers)
            .into_iter()
            .any(|child| text(child, source) == modifier)
    })
}
