mod ast;
mod build_constraints;
mod error_strings;
mod generated;
mod imports;
mod ineffassign;
mod module_pattern;
mod nolint;
mod parentheses;
mod replace_all;
mod resolver;
mod tagged_switch;
pub mod taint;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;

use tree_sitter::Node;

use super::{LintFinding, LintKind, finding_at};
use crate::language::Language;
use crate::limits::Limits;
use crate::rules::{GoRule, RuleId};

pub use error_strings::{CapitalizedError, GoPackageFacts, MessagePiece, PendingMessage, message_problems};
use error_strings::{ErrorStringProblem, error_strings};
pub use nolint::ranges as nolint_ranges;

type GoCheck = fn(Node<'_>, &[u8]) -> Vec<LintFinding>;

const CHECKS: [(GoRule, GoCheck); 4] = [
    (GoRule::Ineffassign, ineffectual_assignments),
    (GoRule::St1005, error_string_findings),
    (GoRule::Qf1003, tagged_switch_findings),
    (GoRule::Qf1004, unlimited_call_findings),
];

// Lo que el resultado toma del archivo además del contenido: si el nombre lo deja fuera de la
// plataforma por defecto.
pub fn analysis_context(language: Language, path: &Path) -> Option<u64> {
    (language == Language::Go).then(|| {
        let mut hasher = DefaultHasher::new();
        analyzed_path(path).hash(&mut hasher);
        hasher.finish()
    })
}

// Si golangci-lint mira el archivo: `./...` lo lista y la plataforma por defecto lo compila.
fn analyzed(path: Option<&Path>, source: &[u8]) -> bool {
    path.is_none_or(analyzed_path) && build_constraints::header_builds(source)
}

fn analyzed_path(path: &Path) -> bool {
    build_constraints::file_name_builds(path) && module_pattern::listed_by_module_pattern(path)
}

pub fn findings(root: Node<'_>, source: &[u8], limits: &Limits, path: Option<&Path>) -> Vec<LintFinding> {
    if root.has_error() || generated::is_generated(root, source) || !analyzed(path, source) {
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
    let mut unresolved = facts.pending.iter().map(PendingMessage::constant_names);
    strings
        .iter()
        .map(|string| {
            let subject = match string.problem {
                ErrorStringProblem::Punctuation => string.callee.clone(),
                ErrorStringProblem::Capitalized => words.next().unwrap_or_default().to_owned(),
                ErrorStringProblem::Unresolved => unresolved.next().unwrap_or_default(),
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
pub fn package_facts(
    root: Node<'_>,
    source: &[u8],
    limits: &Limits,
    path: Option<&Path>,
) -> Option<GoPackageFacts> {
    if limits.get(RuleId::Go(GoRule::St1005)).is_none() || root.has_error() || !analyzed(path, source) {
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
