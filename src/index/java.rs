use std::collections::HashSet;

use tree_sitter::Node;

use super::java_exposure::{DeclaredType, Exposure, exposures};
use super::java_header::{imported_paths, package_annotations, package_of};
use crate::lint::java::VARIABLE_DECLARATOR;
use crate::lint::java_calls::{
    Arity, Body, Call, Callable, ResultUse, ReturnType, Returns, TYPE_DECLARATIONS, Visibility, extract,
};
use crate::lint::java_chains::Chain;
use crate::lint::java_constants::{Constant, declared_constants};
use crate::lint::java_contract_calls::{ContractCall, contract_calls_in};
use crate::lint::java_contracts::{Clause, fail_clauses};
use crate::lint::java_fields::{DeclaredField, declared_fields, used_identifiers, written_only_identifiers};
use crate::lint::java_generics::{Generics, generics_of};
use crate::lint::java_inference::{dereferenced_first, unconditionally_dereferenced};
use crate::lint::java_lambda_targets::{LambdaTargets, lambda_targets_of};
use crate::lint::java_null_flow::NullFlow;
use crate::lint::java_null_inference::{
    NullArgument, accepts_null, is_overridable, null_arguments_in, parameter_type_name, returns_null,
};
use crate::lint::java_owner::{Owner, annotation_names};
use crate::lint::java_receivers::Receiver;
use crate::lint::java_resources::ResourceCandidate;
use crate::lint::java_throws::MethodThrows;
use crate::lint::java_type_variables::{
    has_nullable_qualified_type, has_nullable_type_variable, is_type_variable,
};
use crate::lint::taint::TaintMethod;
use crate::lint::{LintFinding, LintKind};
use crate::rules::JavaRule;
use crate::syntax::descendants;
use crate::syntax::{named_children, one_based};

const BODY_FIELD: &str = "body";
const IDENTIFIER: &str = "identifier";
// Los componentes de un record son sus campos: siempre se usan.
const RECORD: &str = "record_declaration";
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
    pub nullness: ParameterNullness,
    pub dereferenced: Dereferenced,
    pub reading: Reading,
    pub type_name: String,
    pub type_variable: bool,
    pub accepts_null: bool,
}

// Lo que el parámetro dice de su nulabilidad: una anotación (`@Nullable`, `@NonNull`...), nada, o
// nada pero con un tipo que es una variable que admite null (`<T extends @Nullable Object>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterNullness {
    Unannotated,
    Annotated,
    NullableVariable,
}

// Si el cuerpo del método nombra al parámetro (un método sin cuerpo o un record, siempre).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reading {
    Read,
    Unread,
}

// Si el método desreferencia el parámetro en todos los caminos (IntelliJ lo infiere no nulo) y si
// eso es lo primero que hace con él.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dereferenced {
    No,
    Always,
    AlwaysFirst,
}

// Lo que IntelliJ infiere de nulos en un método: nada si se puede redefinir, y si no, si devuelve
// `null` sin un contrato que lo explique.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NullInference {
    Overridable,
    Closed,
    ClosedReturnsNull,
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
    pub returned_arguments: Vec<String>,
    pub inference: NullInference,
    pub parameters: Vec<Parameter>,
    pub annotations: Vec<String>,
    pub owner: Owner,
    pub fail_clauses: Vec<Clause>,
}

// Un campo tal como lo necesita el índice para saber si alguien lo usa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    pub position: Position,
    pub visibility: Visibility,
    pub is_static: bool,
    pub type_name: String,
    pub annotations: Vec<String>,
    pub owner: Owner,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallSite {
    pub name: String,
    pub is_constructor: bool,
    pub receiver: Receiver,
    pub arguments: Vec<Option<String>>,
    pub result: ResultUse,
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
    pub written_only: Vec<String>,
    pub taints: Vec<TaintMethod>,
    pub declared_types: Vec<DeclaredType>,
    pub exposures: Vec<Exposure>,
    pub static_imports: Vec<String>,
    pub method_throws: Vec<MethodThrows>,
    pub null_arguments: Vec<NullArgument>,
    pub contract_calls: Vec<ContractCall>,
    pub generics: Generics,
    pub lambda_targets: LambdaTargets,
}

pub fn summarize(root: Node<'_>, source: &[u8]) -> JavaSummary {
    let (declared_types, exposures) = exposures(root, source);
    let mut summary = JavaSummary {
        generics: generics_of(root, source),
        lambda_targets: lambda_targets_of(root, source),
        declared_types,
        exposures,
        package: package_of(root, source),
        imports: imported_paths(root, source, false),
        static_imports: imported_paths(root, source, true),
        package_annotations: package_annotations(root, source),
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
        summary.taints.extend(extracted.taints);
        summary.method_throws.extend(extracted.method_throws);
        summary
            .null_arguments
            .extend(null_arguments_in(top_level, &nodes, source));
        summary
            .contract_calls
            .extend(contract_calls_in(top_level, &nodes, source));
        summary.constants.extend(declared_constants(&nodes, source));
        summary
            .fields
            .extend(declared_fields(&nodes, source).into_iter().map(field_of));
        summary.identifiers.extend(used_identifiers(&nodes, source));
        summary
            .written_only
            .extend(written_only_identifiers(&nodes, source));
    }
    summary.identifiers.sort_unstable();
    summary.identifiers.dedup();
    summary
}

// Si el parámetro dice algo de su nulabilidad (`@Nullable`, `@NonNull`, `@NotNull`...).
fn declares_nullness(name: Node<'_>, source: &[u8]) -> bool {
    name.parent()
        .and_then(|parent| {
            if parent.kind() == VARIABLE_DECLARATOR {
                parent.parent()
            } else {
                Some(parent)
            }
        })
        .map(|parameter| annotation_names(parameter, source))
        .unwrap_or_default()
        .iter()
        .any(|annotation| NULLNESS_ANNOTATIONS.contains(&annotation.as_str()))
}

fn parameter_nullness(name: Node<'_>, source: &[u8]) -> ParameterNullness {
    if declares_nullness(name, source) || has_nullable_qualified_type(name, source) {
        ParameterNullness::Annotated
    } else if has_nullable_type_variable(name, source) {
        ParameterNullness::NullableVariable
    } else {
        ParameterNullness::Unannotated
    }
}

// Los nombres que se leen en el cuerpo: un parámetro que no aparece no se usa.
fn read_names(callable: &Callable<'_>, source: &[u8]) -> HashSet<String> {
    callable
        .name_node
        .parent()
        .and_then(|declaration| declaration.child_by_field_name(BODY_FIELD))
        .map(descendants)
        .unwrap_or_default()
        .into_iter()
        .filter(|node| node.kind() == IDENTIFIER)
        .filter_map(|node| node.utf8_text(source).ok())
        .map(str::to_owned)
        .collect()
}

fn declaration_of(callable: Callable<'_>, scope: &str, source: &[u8]) -> Declaration {
    let read = read_names(&callable, source);
    let record = callable
        .name_node
        .parent()
        .is_some_and(|declaration| declaration.kind() == RECORD);
    let method = callable.name_node.parent();
    let body = method.and_then(|declaration| declaration.child_by_field_name(BODY_FIELD));
    let dereferenced: HashSet<String> = body
        .map(|body| unconditionally_dereferenced(body, source))
        .unwrap_or_default();
    let used_first: HashSet<String> = body
        .map(|body| dereferenced_first(body, source))
        .unwrap_or_default();
    let inference = if method.is_none_or(is_overridable) {
        NullInference::Overridable
    } else if method.is_some_and(|declaration| returns_null(declaration, source)) {
        NullInference::ClosedReturnsNull
    } else {
        NullInference::Closed
    };
    let fail_clauses = match method {
        Some(declaration) if inference != NullInference::Overridable && callable.body == Body::Written => {
            let not_null: Vec<bool> = callable
                .parameters
                .iter()
                .map(|parameter| {
                    let name = parameter.utf8_text(source).unwrap_or_default();
                    dereferenced.contains(name) && used_first.contains(name)
                })
                .collect();
            fail_clauses(declaration, source, &not_null)
        }
        _ => Vec::new(),
    };
    Declaration {
        fail_clauses,
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
        returned_arguments: callable.returned_arguments,
        inference,
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
                    nullness: parameter_nullness(*parameter, source),
                    dereferenced: match (dereferenced.contains(name), used_first.contains(name)) {
                        (false, _) => Dereferenced::No,
                        (true, false) => Dereferenced::Always,
                        (true, true) => Dereferenced::AlwaysFirst,
                    },
                    reading: if record || callable.body != Body::Written || read.contains(name) {
                        Reading::Read
                    } else {
                        Reading::Unread
                    },
                    type_name: parameter_type_name(*parameter, source),
                    type_variable: is_type_variable(*parameter, source),
                    accepts_null: accepts_null(*parameter),
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
        type_name: field.type_name,
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
        result: call.result,
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
