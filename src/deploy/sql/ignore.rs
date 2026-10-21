use super::lexer::{Comment, comments};
use crate::rules::MigrationRule;

const LINE_COMMENT: &str = "--";
const BLOCK_COMMENT_OPEN: &str = "/*";
const BLOCK_COMMENT_CLOSE: &str = "*/";
const FILE_MARKER: &str = "squawk-ignore-file";
const LINE_MARKER: &str = "squawk-ignore";
const NAME_SEPARATOR: char = ',';
const ALIASES: &[(&str, &[&str])] = &[
    ("prefer-timestamptz", &["prefer-timestamp-tz"]),
    (
        "require-timeout-settings",
        &["require-lock-timeout", "require-statement-timeout"],
    ),
];
const SQUAWK_ONLY_RULES: &[&str] = &[
    "prefer-repack",
    "require-table-schema",
    "ban-drop-trigger",
    "ban-alter-generated-expression",
    "ban-drop-index",
    "ban-set-default",
    "ban-disable-trigger",
    "ban-replica-identity",
    "ban-drop-policy",
    "ban-revoke",
    "ban-replace-view-function",
    "ban-alter-policy-condition",
    "ban-alter-policy-roles",
    "ban-create-policy",
    "ban-alter-function-options",
    "ban-alter-view-options",
    "ban-alter-role-options",
    "ban-alter-database-options",
    "ban-alter-row-level-security",
    "ban-new-write-restriction",
    "ban-add-enum-value",
    "ban-add-composite-attribute",
    "ban-add-column",
    "ban-detach-inheritance",
    "ban-alter-sequence-values",
    "ban-alter-system-options",
    "ban-alter-extension",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SquawkIgnore {
    pub line: usize,
    pub whole_file: bool,
    pub rules: Vec<String>,
    pub unknown: Vec<UnknownName>,
    pub text: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownName {
    pub line: usize,
    pub column: usize,
    pub name: String,
}

pub fn squawk_ignores(source: &str) -> Vec<SquawkIgnore> {
    if !source.contains(LINE_MARKER) {
        return Vec::new();
    }
    comments(source).iter().filter_map(ignore).collect()
}

fn ignore(comment: &Comment) -> Option<SquawkIgnore> {
    let (body, body_start) = comment_body(&comment.text)?;
    let without_start = body.trim_start();
    let (directive, reason) = without_start.trim().split_once(LINE_COMMENT).map_or_else(
        || (without_start.trim(), ""),
        |(directive, reason)| (directive.trim_end(), reason.trim()),
    );
    let (whole_file, names) = directive
        .strip_prefix(FILE_MARKER)
        .map(|names| (true, names))
        .or_else(|| directive.strip_prefix(LINE_MARKER).map(|names| (false, names)))?;
    let names_start = body_start
        .saturating_add(body.len().saturating_sub(without_start.len()))
        .saturating_add(directive.len().saturating_sub(names.len()));
    let mut rules = Vec::new();
    let mut unknown = Vec::new();
    let mut offset = 0_usize;
    for name in names.split(NAME_SEPARATOR) {
        let trimmed = name.trim();
        if !trimmed.is_empty() {
            match expanded(trimmed) {
                Some(known) => rules.extend(known),
                None => unknown.push(UnknownName {
                    line: comment.line,
                    column: comment
                        .column
                        .saturating_add(names_start)
                        .saturating_add(offset)
                        .saturating_add(name.len().saturating_sub(name.trim_start().len())),
                    name: trimmed.to_owned(),
                }),
            }
        }
        offset = offset.saturating_add(name.len()).saturating_add(1);
    }
    Some(SquawkIgnore {
        line: comment.line,
        whole_file,
        rules,
        unknown,
        text: directive.to_owned(),
        reason: reason.to_owned(),
    })
}

fn comment_body(text: &str) -> Option<(&str, usize)> {
    if let Some(body) = text.strip_prefix(LINE_COMMENT) {
        return Some((body, LINE_COMMENT.len()));
    }
    text.strip_prefix(BLOCK_COMMENT_OPEN)
        .and_then(|body| body.strip_suffix(BLOCK_COMMENT_CLOSE))
        .map(|body| (body, BLOCK_COMMENT_OPEN.len()))
}

fn expanded(name: &str) -> Option<Vec<String>> {
    if let Some((_, targets)) = ALIASES.iter().find(|(alias, _)| *alias == name) {
        return Some(targets.iter().map(|target| (*target).to_owned()).collect());
    }
    let known =
        MigrationRule::ALL.iter().any(|rule| rule.as_str() == name) || SQUAWK_ONLY_RULES.contains(&name);
    known.then(|| vec![name.to_owned()])
}
