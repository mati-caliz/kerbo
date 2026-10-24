mod ast;
mod branches;
mod builtins;
mod collection_loops;
mod comments;
mod comparisons;
mod compound;
mod concatenation;
mod container_checks;
mod context_checks;
mod declarations;
mod deprecated;
mod dicts;
mod dunder_all;
mod dunders;
mod encodings;
mod escapes;
mod exception_flow;
mod flow;
mod formats;
mod formatter_comments;
mod fstrings;
mod import_self;
mod imports;
mod known_types;
mod literal_checks;
mod loop_else;
mod loop_lookups;
mod loop_variable_reads;
mod loops;
mod methods;
mod modern;
mod module_names;
mod names;
mod natural;
mod negations;
mod noqa;
mod parentheses;
mod pytest_checks;
mod raises;
mod raw_text;
mod redundancies;
mod rewrites;
mod semantic;
mod set_literals;
mod simplify;
mod slots;
mod special_methods;
mod splits;
mod statement_checks;
mod statements;
mod tail_statements;
pub mod taint;
mod typing_names;

use std::path::Path;

use tree_sitter::{Node, Point};

use super::{LintFinding, LintKind, finding_at};
use crate::fix::Fix;
use crate::limits::{Limits, Preset};
use crate::rules::{PythonRule, RuleId};
use crate::syntax::descendants;

const PACKAGE_INIT_STEM: &str = "__init__";
const STUB_EXTENSION: &str = "pyi";
const MINIMAL_OFF: [PythonRule; 16] = [
    PythonRule::I001,
    PythonRule::E401,
    PythonRule::E402,
    PythonRule::E701,
    PythonRule::E702,
    PythonRule::E703,
    PythonRule::E711,
    PythonRule::E712,
    PythonRule::E713,
    PythonRule::E714,
    PythonRule::E721,
    PythonRule::E722,
    PythonRule::E731,
    PythonRule::E741,
    PythonRule::E742,
    PythonRule::E743,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FileKind {
    pub package_init: bool,
    pub stub: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub rule: PythonRule,
    pub start: Point,
    pub subject: String,
    pub fix: Option<Fix>,
    pub statement_row: Option<usize>,
}

impl Report {
    pub fn at(rule: PythonRule, node: Node<'_>, subject: &str) -> Self {
        Self::at_point(rule, node.start_position(), subject)
    }

    pub fn at_point(rule: PythonRule, start: Point, subject: &str) -> Self {
        Self {
            rule,
            start,
            subject: subject.to_owned(),
            fix: None,
            statement_row: None,
        }
    }

    pub fn with_fix(mut self, fix: Option<Fix>) -> Self {
        self.fix = fix;
        self
    }

    pub const fn with_statement_row(mut self, row: usize) -> Self {
        self.statement_row = Some(row);
        self
    }
}

pub fn relaxed_by(preset: Preset) -> Vec<RuleId> {
    let relaxed: &[PythonRule] = match preset {
        Preset::Strict | Preset::Recommended => &[],
        Preset::Minimal => &MINIMAL_OFF,
    };
    relaxed.iter().copied().map(RuleId::Python).collect()
}

pub fn findings(root: Node<'_>, source: &[u8], limits: Limits, path: Option<&Path>) -> Vec<LintFinding> {
    let enabled = |rule: PythonRule| limits.get(RuleId::Python(rule)).is_some();
    if !PythonRule::ALL.into_iter().any(enabled) {
        return Vec::new();
    }
    let file = file_kind(path);
    let nodes = descendants(root);
    let mut reports = if root.has_error() {
        Vec::new()
    } else {
        let import_file = imports::ImportFile {
            source,
            path,
            stub: file.stub,
        };
        let mut found = semantic::reports(root, source, file);
        found.extend(imports::reports(root, &nodes, &import_file));
        found
    };
    reports.extend(compound::reports(root, source));
    reports.extend(statements::reports(&nodes, source));
    reports.extend(flow::reports(&nodes));
    reports.extend(names::reports(&nodes, source));
    reports.extend(comparisons::reports(&nodes, source));
    reports.extend(fstrings::reports(&nodes, source));
    reports.extend(formats::reports(&nodes, source));
    reports.extend(dicts::reports(&nodes, source));
    reports.extend(modern::reports(root, &nodes, source));
    reports.extend(simplify::reports(&nodes, source));
    reports.extend(splits::reports(&nodes, source));
    reports.extend(slots::reports(&nodes, source));
    reports.extend(loop_else::reports(&nodes));
    reports.extend(parentheses::reports(root));
    reports.extend(declarations::reports(&nodes, source));
    reports.extend(raises::reports(&nodes, source));
    reports.extend(encodings::reports(&nodes, source));
    reports.extend(redundancies::reports(&nodes, source));
    reports.extend(set_literals::reports(&nodes, source));
    reports.extend(literal_checks::reports(&nodes, source));
    reports.extend(special_methods::reports(&nodes, source));
    reports.extend(statement_checks::reports(&nodes, source));
    reports.extend(collection_loops::reports(&nodes, source));
    reports.extend(pytest_checks::reports(&nodes, source));
    reports.extend(tail_statements::reports(&nodes, source));
    reports.extend(loop_lookups::reports(&nodes, source));
    reports.extend(container_checks::reports(&nodes, source));
    reports.extend(context_checks::reports(&nodes, source));
    reports.extend(dunder_all::reports(&nodes, source));
    reports.extend(raw_text::reports(&nodes, source));
    reports.extend(import_self::reports(&nodes, source, path));
    reports.extend(typing_names::reports(&nodes, source));
    reports.extend(exception_flow::reports(&nodes, source));
    reports.extend(methods::reports(&nodes, source));
    reports.extend(loops::loop_variable_overrides(&nodes, source));
    reports.extend(loops::iteration_over_set(&nodes, source));
    reports.extend(deprecated::reports(&nodes, source));
    reports.extend(concatenation::reports(&nodes));
    reports.extend(branches::reports(&nodes, source));
    reports.extend(loops::reports(&nodes, source));
    reports.extend(dunders::reports(&nodes, source));
    reports.extend(negations::reports(&nodes, source));
    reports.extend(comments::reports(&nodes, source));
    reports.extend(formatter_comments::reports(root, &nodes, source));
    reports.extend(rewrites::reports(&nodes, source));
    reports.extend(module_names::invalid_module_name(path));
    let mut found: Vec<LintFinding> = suppressed(root, source, reports, &enabled)
        .into_iter()
        .map(|report| {
            finding_at(
                LintKind::Python(report.rule),
                report.start,
                &report.subject,
                report.fix,
            )
        })
        .collect();
    found.sort_by_key(|finding| (finding.line, finding.column));
    found
}

// Lo que queda después de los `noqa`, más los `noqa` sin uso (RUF100) que se calculan con los avisos
// de las reglas prendidas antes de silenciarlos.
fn suppressed(
    root: Node<'_>,
    source: &[u8],
    mut reports: Vec<Report>,
    enabled: &dyn Fn(PythonRule) -> bool,
) -> Vec<Report> {
    let suppressions = noqa::Suppressions::new(root, source);
    reports.retain(|report| enabled(report.rule));
    let unused = noqa::unused_directives(&suppressions, &reports, enabled, !root.has_error());
    reports.retain(|report| !suppressions.silences(report));
    reports.extend(unused.into_iter().filter(|report| enabled(report.rule)));
    reports
}

fn file_kind(path: Option<&Path>) -> FileKind {
    FileKind {
        package_init: path
            .and_then(Path::file_stem)
            .is_some_and(|stem| stem == PACKAGE_INIT_STEM),
        stub: path
            .and_then(Path::extension)
            .is_some_and(|extension| extension == STUB_EXTENSION),
    }
}

#[cfg(test)]
mod tests;
