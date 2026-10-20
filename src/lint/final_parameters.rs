use tree_sitter::Node;

use super::java::{
    BODY_FIELD, CONSTRUCTOR, FINAL, METHOD, PARAMETERS_FIELD, UNNAMED, has_modifier, parameter_name,
    parameter_type, parameters, text,
};
use super::recovered::RecoveredParameter;
use super::{LintFinding, LintKind, finding, finding_at, recovered};
use crate::fix::Fix;
use crate::syntax::descendants;

const FINAL_PREFIX: &str = "final ";

pub fn reported(root: Node<'_>, source: &[u8]) -> Vec<LintFinding> {
    descendants(root)
        .into_iter()
        .filter(|node| [METHOD, CONSTRUCTOR].contains(&node.kind()))
        .filter(|declaration| declaration.child_by_field_name(BODY_FIELD).is_some())
        .flat_map(
            |declaration| match declaration.child_by_field_name(PARAMETERS_FIELD) {
                Some(list) if list.has_error() => recovered_findings(list, source),
                _ => parsed_findings(declaration, source),
            },
        )
        .collect()
}

fn parsed_findings(declaration: Node<'_>, source: &[u8]) -> Vec<LintFinding> {
    parameters(declaration)
        .into_iter()
        .filter(|parameter| !has_modifier(*parameter, FINAL))
        .filter_map(|parameter| {
            let name = text(parameter_name(parameter)?, source);
            (name != UNNAMED).then(|| {
                let fix =
                    parameter_type(parameter).map(|kind| Fix::insert_before(kind, FINAL_PREFIX, source));
                finding(LintKind::FinalParameter, parameter, name, fix)
            })
        })
        .collect()
}

fn recovered_findings(list: Node<'_>, source: &[u8]) -> Vec<LintFinding> {
    let close = list.end_byte().saturating_sub(1);
    recovered::parameters(source, list.start_byte(), close)
        .into_iter()
        .filter(|parameter| !parameter.is_final && parameter.name != UNNAMED)
        .map(
            |RecoveredParameter {
                 start,
                 type_start,
                 name,
                 ..
             }| {
                let fix = Fix::insert_at(type_start, FINAL_PREFIX, source);
                finding_at(LintKind::FinalParameter, start, &name, Some(fix))
            },
        )
        .collect()
}
