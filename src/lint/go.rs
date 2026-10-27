mod ast;
mod error_strings;
mod generated;
mod imports;
mod ineffassign;
mod nolint;
mod parentheses;
mod replace_all;
mod resolver;
mod tagged_switch;
pub mod taint;

use tree_sitter::Node;

use super::{LintFinding, LintKind, finding_at};
use crate::limits::Limits;
use crate::rules::{GoRule, RuleId};

pub use error_strings::{CapitalizedError, GoPackageFacts};
use error_strings::{ErrorStringProblem, error_strings};
pub use nolint::ranges as nolint_ranges;

type GoCheck = fn(Node<'_>, &[u8]) -> Vec<LintFinding>;

const CHECKS: [(GoRule, GoCheck); 4] = [
    (GoRule::Ineffassign, ineffectual_assignments),
    (GoRule::St1005, error_string_findings),
    (GoRule::Qf1003, tagged_switch_findings),
    (GoRule::Qf1004, unlimited_call_findings),
];

pub fn findings(root: Node<'_>, source: &[u8], limits: &Limits) -> Vec<LintFinding> {
    if root.has_error() || generated::is_generated(root, source) {
        return Vec::new();
    }
    CHECKS
        .into_iter()
        .filter(|(rule, _)| limits.get(RuleId::Go(*rule)).is_some())
        .flat_map(|(_, check)| check(root, source))
        .collect()
}

fn ineffectual_assignments(root: Node<'_>, source: &[u8]) -> Vec<LintFinding> {
    let file = ast::file(root, source);
    let resolution = resolver::resolve(&file);
    ineffassign::ineffectual(&file, &resolution)
        .into_iter()
        .filter_map(|ident| file.ident(ident))
        .map(|ident| finding_at(LintKind::Go(GoRule::Ineffassign), ident.start, &ident.name, None))
        .collect()
}

fn error_string_findings(root: Node<'_>, source: &[u8]) -> Vec<LintFinding> {
    let (strings, facts) = error_strings(root, source);
    let mut words = facts
        .capitalized
        .iter()
        .map(|capitalized| capitalized.word.as_str());
    strings
        .iter()
        .map(|string| {
            let subject = match string.problem {
                ErrorStringProblem::Punctuation => string.callee.clone(),
                ErrorStringProblem::Capitalized => words.next().unwrap_or_default().to_owned(),
            };
            finding_at(
                LintKind::Go(GoRule::St1005),
                string.call.start_position(),
                &subject,
                None,
            )
        })
        .collect()
}

fn tagged_switch_findings(root: Node<'_>, source: &[u8]) -> Vec<LintFinding> {
    tagged_switch::tagged_switches(root, source)
        .into_iter()
        .map(|(statement, tag)| {
            finding_at(
                LintKind::Go(GoRule::Qf1003),
                statement.start_position(),
                &tag,
                None,
            )
        })
        .collect()
}

fn unlimited_call_findings(root: Node<'_>, source: &[u8]) -> Vec<LintFinding> {
    replace_all::unlimited_calls(root, source)
        .into_iter()
        .map(|(function, replacement)| {
            finding_at(
                LintKind::Go(GoRule::Qf1004),
                function.start_position(),
                replacement,
                None,
            )
        })
        .collect()
}

// Lo que el archivo aporta a ST1005 en su paquete: los mensajes con mayúscula y lo que declara. Un
// archivo generado no avisa pero sus nombres cuentan, como en golangci-lint.
pub fn package_facts(root: Node<'_>, source: &[u8], limits: &Limits) -> Option<GoPackageFacts> {
    if limits.get(RuleId::Go(GoRule::St1005)).is_none() || root.has_error() {
        return None;
    }
    let mut facts = error_strings(root, source).1;
    if generated::is_generated(root, source) {
        facts.capitalized.clear();
        facts.outside_functions.clear();
    }
    Some(facts)
}

#[cfg(test)]
mod tests;
