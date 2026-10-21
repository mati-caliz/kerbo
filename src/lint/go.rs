mod ast;
mod generated;
mod ineffassign;
mod nolint;
mod resolver;

use tree_sitter::Node;

use super::{LintFinding, LintKind, finding_at};
use crate::limits::Limits;
use crate::rules::{GoRule, RuleId};

pub use nolint::ranges as nolint_ranges;

pub fn findings(root: Node<'_>, source: &[u8], limits: Limits) -> Vec<LintFinding> {
    let enabled = |rule: GoRule| limits.get(RuleId::Go(rule)).is_some();
    if !GoRule::ALL.into_iter().any(enabled) || root.has_error() || generated::is_generated(root, source) {
        return Vec::new();
    }
    let file = ast::file(root, source);
    let resolution = resolver::resolve(&file);
    ineffassign::ineffectual(&file, &resolution)
        .into_iter()
        .filter_map(|ident| file.ident(ident))
        .map(|ident| finding_at(LintKind::Go(GoRule::Ineffassign), ident.start, &ident.name, None))
        .collect()
}

#[cfg(test)]
mod tests;
