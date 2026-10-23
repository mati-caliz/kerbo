mod ast;
mod error_strings;
mod generated;
mod ineffassign;
mod nolint;
mod resolver;
pub mod taint;

use tree_sitter::Node;

use super::{LintFinding, LintKind, finding_at};
use crate::limits::Limits;
use crate::rules::{GoRule, RuleId};

pub use error_strings::{CapitalizedError, GoPackageFacts};
use error_strings::{ErrorStringProblem, error_strings};
pub use nolint::ranges as nolint_ranges;

fn analyzable(root: Node<'_>, source: &[u8], limits: Limits, rules: &[GoRule]) -> bool {
    rules.iter().any(|rule| limits.get(RuleId::Go(*rule)).is_some())
        && !root.has_error()
        && !generated::is_generated(root, source)
}

pub fn findings(root: Node<'_>, source: &[u8], limits: Limits) -> Vec<LintFinding> {
    let mut found = Vec::new();
    if analyzable(root, source, limits, &[GoRule::Ineffassign]) {
        let file = ast::file(root, source);
        let resolution = resolver::resolve(&file);
        found.extend(
            ineffassign::ineffectual(&file, &resolution)
                .into_iter()
                .filter_map(|ident| file.ident(ident))
                .map(|ident| finding_at(LintKind::Go(GoRule::Ineffassign), ident.start, &ident.name, None)),
        );
    }
    if analyzable(root, source, limits, &[GoRule::St1005]) {
        let (strings, facts) = error_strings(root, source);
        let mut words = facts
            .capitalized
            .iter()
            .map(|capitalized| capitalized.word.as_str());
        found.extend(strings.iter().map(|string| {
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
        }));
    }
    found
}

// Lo que el archivo aporta a ST1005 en su paquete: los mensajes con mayúscula y lo que declara. Un
// archivo generado no avisa pero sus nombres cuentan, como en golangci-lint.
pub fn package_facts(root: Node<'_>, source: &[u8], limits: Limits) -> Option<GoPackageFacts> {
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
