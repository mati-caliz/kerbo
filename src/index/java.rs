use std::collections::HashSet;

use tree_sitter::Node;

use crate::lint::java::children;
use crate::lint::java_calls::{
    Arity, Body, Call, Callable, ReturnType, Returns, TYPE_DECLARATIONS, Visibility, extract,
};
use crate::lint::java_chains::Chain;
use crate::lint::java_constants::{Constant, declared_constants};
use crate::lint::java_fields::{DeclaredField, declared_fields, used_identifiers};
use crate::lint::java_inference::unconditionally_dereferenced;
use crate::lint::java_null_flow::NullFlow;
use crate::lint::java_owner::{Owner, annotation_names};
use crate::lint::java_receivers::Receiver;
use crate::lint::java_resources::ResourceCandidate;
use crate::lint::{LintFinding, LintKind};
use crate::rules::JavaRule;
use crate::syntax::descendants;
use crate::syntax::{named_children, one_based};

const PACKAGE: &str = "package_declaration";
const ANNOTATION: &str = "marker_annotation";
const ANNOTATIONS: &[&str] = &["marker_annotation", "annotation"];
const IMPORT: &str = "import_declaration";
const STATIC: &str = "static";
const ASTERISK: &str = "asterisk";
const WILDCARD_SUFFIX: &str = ".*";
const NAME_FIELD: &str = "name";
const BODY_FIELD: &str = "body";
const QUALIFIER_SEPARATOR: char = '.';
const NULLNESS_ANNOTATIONS: &[&str] = &["Nullable", "NonNull", "NotNull", "Nonnull", "CheckForNull"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    pub name: String,
    pub position: Position,
    pub declares_nullness: bool,
    pub dereferenced: bool,
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
    pub return_type: ReturnType,
    pub returned: String,
    pub parameters: Vec<Parameter>,
    pub annotations: Vec<String>,
    pub owner: Owner,
}

// Un campo tal como lo necesita el índice para saber si alguien lo usa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    pub position: Position,
    pub visibility: Visibility,
    pub is_static: bool,
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
    pub imports: Vec<String>,
    pub package_annotations: Vec<String>,
    pub chains: Vec<Chain>,
    pub null_flows: Vec<NullFlow>,
    pub resources: Vec<ResourceCandidate>,
    pub fields: Vec<Field>,
    pub identifiers: Vec<String>,
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
        imports: named_children(root)
            .into_iter()
            .filter(|node| node.kind() == IMPORT)
            .filter(|import| !children(*import).iter().any(|child| child.kind() == STATIC))
            .filter_map(|import| imported_path(import, source))
            .collect(),
        package_annotations: named_children(root)
            .into_iter()
            .find(|node| node.kind() == PACKAGE)
            .map(|package| package_annotation_names(package, source))
            .unwrap_or_default(),
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
        let nodes = descendants(top_level);
        let extracted = extract(top_level, source);
        summary.declarations.extend(
            extracted
                .callables
                .into_iter()
                .map(|callable| declaration_of(callable, &scope, source)),
        );
        summary.calls.extend(extracted.calls.into_iter().map(call_site));
        summary.referenced.extend(extracted.referenced);
        summary.types.extend(extracted.types);
        summary.mentioned.extend(extracted.mentioned);
        summary.hierarchy.extend(extracted.hierarchy);
        summary.chains.extend(extracted.chains);
        summary.null_flows.extend(extracted.null_flows);
        summary.resources.extend(extracted.resources);
        summary.constants.extend(declared_constants(&nodes, source));
        summary
            .fields
            .extend(declared_fields(&nodes, source).into_iter().map(field_of));
        summary.identifiers.extend(used_identifiers(&nodes, source));
    }
    summary.identifiers.sort_unstable();
    summary.identifiers.dedup();
    summary
}

// `import a.b.C;` da `a.b.C` y `import a.b.*;` da `a.b.*`.
fn imported_path(import: Node<'_>, source: &[u8]) -> Option<String> {
    let parts = named_children(import);
    let path = parts.first()?.utf8_text(source).ok()?;
    let wildcard = parts.iter().any(|part| part.kind() == ASTERISK);
    Some(if wildcard {
        format!("{path}{WILDCARD_SUFFIX}")
    } else {
        path.to_owned()
    })
}

fn package_annotation_names(package: Node<'_>, source: &[u8]) -> Vec<String> {
    named_children(package)
        .into_iter()
        .filter(|child| ANNOTATIONS.contains(&child.kind()))
        .filter_map(|annotation| annotation.child_by_field_name(NAME_FIELD))
        .filter_map(|name| name.utf8_text(source).ok())
        .map(simple_name)
        .collect()
}

// Si el parámetro dice algo de su nulabilidad (`@Nullable`, `@NonNull`, `@NotNull`...).
fn declares_nullness(name: Node<'_>, source: &[u8]) -> bool {
    name.parent()
        .map(|parameter| annotation_names(parameter, source))
        .unwrap_or_default()
        .iter()
        .any(|annotation| NULLNESS_ANNOTATIONS.contains(&annotation.as_str()))
}

fn simple_name(written: &str) -> String {
    written
        .rsplit(QUALIFIER_SEPARATOR)
        .next()
        .unwrap_or(written)
        .to_owned()
}

fn declaration_of(callable: Callable<'_>, scope: &str, source: &[u8]) -> Declaration {
    let dereferenced = if callable.overrides {
        callable
            .name_node
            .parent()
            .and_then(|declaration| declaration.child_by_field_name(BODY_FIELD))
            .map(|body| unconditionally_dereferenced(body, source))
            .unwrap_or_default()
    } else {
        HashSet::new()
    };
    Declaration {
        name: callable.name,
        scope: scope.to_owned(),
        position: position_of(callable.name_node),
        is_constructor: callable.is_constructor,
        visibility: callable.visibility,
        overrides: callable.overrides,
        body: callable.body,
        arity: callable.arity,
        returns: callable.returns,
        return_type: callable.return_type,
        returned: callable.returned,
        annotations: callable.annotations,
        owner: callable.owner,
        parameters: callable
            .parameters
            .iter()
            .map(|parameter| {
                let name = parameter.utf8_text(source).unwrap_or_default();
                Parameter {
                    name: name.to_owned(),
                    position: position_of(*parameter),
                    declares_nullness: declares_nullness(*parameter, source),
                    dereferenced: dereferenced.contains(name),
                }
            })
            .collect(),
    }
}

fn field_of(field: DeclaredField<'_>) -> Field {
    Field {
        name: field.name,
        position: position_of(field.name_node),
        visibility: field.visibility,
        is_static: field.is_static,
        annotations: field.annotations,
        owner: field.owner,
    }
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
