use crate::i18n::Lang;
use crate::rules::{Diagnostic, RuleId};

mod json;
mod rows;
mod short_id;

pub(super) use json::document;
pub(super) use rows::write;

const SHOWN_LIMIT: usize = 30;
const EXPLAIN_HINT: &str = "kerbo explain <rule>";
const CHEATING_RULES: [RuleId; 4] = [
    RuleId::SuppressionAdded,
    RuleId::TestDisabled,
    RuleId::TestRemoved,
    RuleId::TypeEscape,
];

#[derive(Debug)]
pub(super) struct Report<'diagnostic> {
    problems: usize,
    files: Vec<FileGroup<'diagnostic>>,
    fixes: Vec<RuleFix>,
}

#[derive(Debug)]
struct FileGroup<'diagnostic> {
    path: &'diagnostic str,
    rows: Vec<Row<'diagnostic>>,
}

#[derive(Debug)]
struct Row<'diagnostic> {
    id: String,
    diagnostic: &'diagnostic Diagnostic,
}

#[derive(Debug)]
struct RuleFix {
    rule: RuleId,
    text: &'static str,
}

impl Report<'_> {
    fn shown(&self) -> usize {
        self.files.iter().map(|file| file.rows.len()).sum()
    }

    const fn more(&self) -> usize {
        self.problems.saturating_sub(SHOWN_LIMIT)
    }
}

pub(super) fn build(diagnostics: &[Diagnostic], lang: Lang) -> Report<'_> {
    let fingerprints: Vec<&str> = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.fingerprint.as_str())
        .collect();
    let mut ordered: Vec<Row<'_>> = diagnostics
        .iter()
        .zip(short_id::shortest_unique(&fingerprints))
        .map(|(diagnostic, id)| Row { id, diagnostic })
        .collect();
    ordered.sort_by_key(|row| std::cmp::Reverse(priority(row.diagnostic)));
    ordered.truncate(SHOWN_LIMIT);
    let fixes = fixes(&ordered, lang);
    Report {
        problems: diagnostics.len(),
        files: group_by_file(ordered),
        fixes,
    }
}

fn priority(diagnostic: &Diagnostic) -> (bool, usize) {
    let cheating = CHEATING_RULES.contains(&diagnostic.rule);
    let excess = diagnostic.value.saturating_sub(diagnostic.limit);
    (cheating, excess)
}

fn fixes(rows: &[Row<'_>], lang: Lang) -> Vec<RuleFix> {
    let mut rules: Vec<RuleId> = Vec::new();
    for row in rows {
        if !rules.contains(&row.diagnostic.rule) {
            rules.push(row.diagnostic.rule);
        }
    }
    rules
        .into_iter()
        .map(|rule| RuleFix {
            rule,
            text: lang.explain(rule).fix,
        })
        .collect()
}

fn group_by_file(rows: Vec<Row<'_>>) -> Vec<FileGroup<'_>> {
    let mut files: Vec<FileGroup<'_>> = Vec::new();
    for row in rows {
        let path = row.diagnostic.relative_path.as_str();
        match files.iter_mut().find(|file| file.path == path) {
            Some(file) => file.rows.push(row),
            None => files.push(FileGroup {
                path,
                rows: vec![row],
            }),
        }
    }
    files
}

#[cfg(test)]
mod tests;
