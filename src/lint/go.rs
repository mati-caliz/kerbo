mod ast;
mod build_constraints;
mod errcheck;
mod error_strings;
mod generated;
mod imports;
mod ineffassign;
mod module_pattern;
mod nolint;
mod parentheses;
pub mod platform;
mod replace_all;
mod resolver;
mod tagged_switch;
pub mod taint;
mod typed_candidates;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;

use tree_sitter::Node;

use super::{LintFinding, LintKind, finding_at};
use crate::analysis::SourceUnit;
use crate::language::Language;
use crate::limits::Limits;
use crate::rules::{GoRule, RuleId};

pub use error_strings::{CapitalizedError, GoPackageFacts, MessagePiece, PendingMessage, message_problems};
use error_strings::{ErrorStringProblem, error_strings};
pub use module_pattern::module_root;
pub use nolint::ranges as nolint_ranges;
use platform::GoPlatform;

type GoCheck = fn(Node<'_>, &[u8]) -> Vec<LintFinding>;

const CHECKS: [(GoRule, GoCheck); 7] = [
    (GoRule::Ineffassign, ineffectual_assignments),
    (GoRule::Errcheck, unchecked_error_candidates),
    (GoRule::St1005, error_string_findings),
    (GoRule::Qf1003, tagged_switch_findings),
    (GoRule::Qf1004, unlimited_call_findings),
    (GoRule::Qf1008, embedded_field_findings),
    (GoRule::Qf1012, fprint_findings),
];

// Lo que el resultado toma del archivo además del contenido: la plataforma y si el nombre o la
// carpeta lo dejan afuera.
pub fn analysis_context(language: Language, path: &Path, platform: &GoPlatform) -> Option<u64> {
    (language == Language::Go).then(|| {
        let mut hasher = DefaultHasher::new();
        platform.hash(&mut hasher);
        analyzed_path(path, platform).hash(&mut hasher);
        hasher.finish()
    })
}

// Si golangci-lint mira el archivo: `./...` lo lista y la plataforma lo compila. Sin plataforma
// (los tests), linux/amd64.
fn analyzed(unit: SourceUnit<'_>) -> bool {
    let default_platform = GoPlatform::default();
    let platform = unit.go_platform.unwrap_or(&default_platform);
    unit.path.is_none_or(|path| analyzed_path(path, platform))
        && build_constraints::header_builds(unit.source, platform)
}

fn analyzed_path(path: &Path, platform: &GoPlatform) -> bool {
    build_constraints::file_name_builds(path, platform) && module_pattern::listed_by_module_pattern(path)
}

// Si un archivo Go es generado, como lo decide golangci-lint, sin tener el árbol a mano.
pub fn is_generated_source(source: &[u8]) -> bool {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_go::LANGUAGE.into()).is_ok()
        && parser
            .parse(source, None)
            .is_some_and(|tree| generated::is_generated(tree.root_node(), source))
}

pub fn findings(root: Node<'_>, unit: SourceUnit<'_>, limits: &Limits) -> Vec<LintFinding> {
    let source = unit.source;
    if root.has_error() || generated::is_generated(root, source) || !analyzed(unit) {
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

fn unchecked_error_candidates(root: Node<'_>, source: &[u8]) -> Vec<LintFinding> {
    errcheck::candidates(root, source)
        .into_iter()
        .map(|candidate| {
            finding_at(
                LintKind::Go(GoRule::Errcheck),
                candidate.position,
                &candidate.subject,
                None,
            )
        })
        .collect()
}

fn embedded_field_findings(root: Node<'_>, source: &[u8]) -> Vec<LintFinding> {
    typed_candidates::embedded_field_candidates(root, source)
        .into_iter()
        .map(|(position, field)| finding_at(LintKind::Go(GoRule::Qf1008), position, field, None))
        .collect()
}

fn fprint_findings(root: Node<'_>, source: &[u8]) -> Vec<LintFinding> {
    typed_candidates::fprint_candidates(root, source)
        .into_iter()
        .map(|position| finding_at(LintKind::Go(GoRule::Qf1012), position, "", None))
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
pub fn package_facts(root: Node<'_>, unit: SourceUnit<'_>, limits: &Limits) -> Option<GoPackageFacts> {
    let source = unit.source;
    if limits.get(RuleId::Go(GoRule::St1005)).is_none() || root.has_error() || !analyzed(unit) {
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
