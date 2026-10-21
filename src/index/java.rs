use tree_sitter::Node;

use crate::lint::java_calls::{Arity, Body, Call, Returns, TYPE_DECLARATIONS, Visibility, extract};
use crate::lint::java_constants::{Constant, declared_constants};
use crate::lint::java_owner::Owner;
use crate::lint::java_receivers::Receiver;
use crate::lint::{LintFinding, LintKind};
use crate::rules::JavaRule;
use crate::syntax::descendants;
use crate::syntax::{named_children, one_based};

const PACKAGE: &str = "package_declaration";
const ANNOTATION: &str = "marker_annotation";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    pub name: String,
    pub position: Position,
}

// Un método o constructor tal como lo necesita el índice: sin nodos, para guardarlo en la caché.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declaration {
    pub name: String,
    pub scope: String,
    pub position: Position,
    pub is_constructor: bool,
    pub visibility: Visibility,
    pub overrides: bool,
    pub body: Body,
    pub arity: Arity,
    pub returns: Returns,
    pub parameters: Vec<Parameter>,
    pub annotations: Vec<String>,
    pub owner: Owner,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallSite {
    pub name: String,
    pub is_constructor: bool,
    pub receiver: Receiver,
    pub arguments: Vec<Option<String>>,
    pub result_used: bool,
}

// Lo que un archivo Java declara y llama, para cruzarlo con el resto del proyecto.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JavaSummary {
    pub declarations: Vec<Declaration>,
    pub calls: Vec<CallSite>,
    pub referenced: Vec<String>,
    pub types: Vec<String>,
    pub package: String,
    pub constants: Vec<Constant>,
    pub mentioned: Vec<String>,
    pub hierarchy: Vec<(String, Vec<String>)>,
}

pub fn summarize(root: Node<'_>, source: &[u8]) -> JavaSummary {
    let mut summary = JavaSummary {
        package: named_children(root)
            .into_iter()
            .find(|node| node.kind() == PACKAGE)
            .and_then(|package| {
                named_children(package)
                    .into_iter()
                    .find(|child| child.kind() != ANNOTATION)
            })
            .and_then(|name| name.utf8_text(source).ok())
            .unwrap_or_default()
            .to_owned(),
        ..JavaSummary::default()
    };
    for top_level in named_children(root)
        .into_iter()
        .filter(|node| TYPE_DECLARATIONS.contains(&node.kind()))
    {
        let scope = top_level
            .child_by_field_name("name")
            .and_then(|name| name.utf8_text(source).ok())
            .unwrap_or_default()
            .to_owned();
        let extracted = extract(top_level, source);
        summary
            .declarations
            .extend(extracted.callables.into_iter().map(|callable| {
                Declaration {
                    name: callable.name,
                    scope: scope.clone(),
                    position: position_of(callable.name_node),
                    is_constructor: callable.is_constructor,
                    visibility: callable.visibility,
                    overrides: callable.overrides,
                    body: callable.body,
                    arity: callable.arity,
                    returns: callable.returns,
                    annotations: callable.annotations,
                    owner: callable.owner,
                    parameters: callable
                        .parameters
                        .iter()
                        .map(|parameter| Parameter {
                            name: parameter.utf8_text(source).unwrap_or_default().to_owned(),
                            position: position_of(*parameter),
                        })
                        .collect(),
                }
            }));
        summary.calls.extend(extracted.calls.into_iter().map(call_site));
        summary.referenced.extend(extracted.referenced);
        summary.types.extend(extracted.types);
        summary.mentioned.extend(extracted.mentioned);
        summary.hierarchy.extend(extracted.hierarchy);
        summary
            .constants
            .extend(declared_constants(&descendants(top_level), source));
    }
    summary
}

fn call_site(call: Call) -> CallSite {
    CallSite {
        name: call.name,
        is_constructor: call.is_constructor,
        receiver: call.receiver,
        arguments: call.arguments,
        result_used: call.result_used,
    }
}

fn position_of(node: Node<'_>) -> Position {
    let start = node.start_position();
    Position {
        line: one_based(start.row),
        column: one_based(start.column),
    }
}

pub const fn lint_at(rule: JavaRule, position: &Position, subject: String) -> LintFinding {
    LintFinding {
        kind: LintKind::Java(rule),
        line: position.line,
        column: position.column,
        subject,
        fix: None,
    }
}
