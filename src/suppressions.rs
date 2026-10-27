use std::collections::HashMap;
use std::path::Path;

use crate::deploy::squawk_ignores;
use crate::language::Language;
use crate::lint::{DisabledRange, eslint_disabled_ranges, go_nolint_ranges};
use crate::rules::{Diagnostic, RuleId};
use crate::shell::{self, Coverage};

const CODE_SEPARATOR: char = ',';
const REASON_LEADERS: &[char] = &[' ', '\t', '#', '-', ':', '—'];
const FIRST_LINE: usize = 1;
const REPEATED_SHELL_DISABLE: &str = "disable=";
const GO_EXTENSION: &str = ".go";
const SQUAWK_NEXT_LINES: usize = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    NextLine,
    File,
    ShellStatement,
    GoNolint,
    SquawkLines,
    SquawkFile,
    EslintRange { first_column: usize, last_column: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Syntax {
    pub comment: &'static str,
    pub marker: &'static str,
    pub scope: Scope,
}

pub const SYNTAXES: [Syntax; 3] = [
    Syntax {
        comment: "#",
        marker: "hadolint global ignore=",
        scope: Scope::File,
    },
    Syntax {
        comment: "#",
        marker: "hadolint ignore=",
        scope: Scope::NextLine,
    },
    Syntax {
        comment: "#",
        marker: "shellcheck disable=",
        scope: Scope::ShellStatement,
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Directive {
    pub line: usize,
    pub text: String,
    pub scope: Scope,
    pub codes: Vec<String>,
    pub reason: String,
    pub covered_line: Option<usize>,
    pub last_covered_line: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct Suppressed {
    pub diagnostic: Diagnostic,
    pub reason: String,
}

impl Directive {
    fn covers(&self, diagnostic: &Diagnostic) -> bool {
        let squawk = matches!(self.scope, Scope::SquawkLines | Scope::SquawkFile);
        if squawk && !matches!(diagnostic.rule, RuleId::Migration(_)) {
            return false;
        }
        if let Scope::EslintRange {
            first_column,
            last_column,
        } = self.scope
        {
            return self.covers_position(diagnostic, first_column, last_column);
        }
        let in_scope = match self.scope {
            Scope::File | Scope::SquawkFile => true,
            Scope::NextLine => self.covered_line == Some(diagnostic.line),
            Scope::GoNolint if !diagnostic.path.ends_with(GO_EXTENSION) => false,
            Scope::ShellStatement | Scope::GoNolint | Scope::SquawkLines | Scope::EslintRange { .. } => self
                .covered_line
                .zip(self.last_covered_line)
                .is_some_and(|(first, last)| (first..=last).contains(&diagnostic.line)),
        };
        in_scope
            && ((matches!(self.scope, Scope::GoNolint | Scope::SquawkFile) && self.codes.is_empty())
                || self.codes.iter().any(|code| {
                    diagnostic.rule.as_str().eq_ignore_ascii_case(code)
                        || (self.scope == Scope::ShellStatement && shell::disables(code, diagnostic.rule))
                        || (self.scope == Scope::GoNolint
                            && matches!(diagnostic.rule, RuleId::Go(rule) if rule.linter() == code))
                }))
    }

    // Como `applyDisableDirectives` de ESLint: desde la posición del comentario hasta antes de la del
    // `eslint-enable`, con la lista vacía para todas las reglas.
    fn covers_position(&self, diagnostic: &Diagnostic, first_column: usize, last_column: usize) -> bool {
        let Some((first_line, last_line)) = self.covered_line.zip(self.last_covered_line) else {
            return false;
        };
        let at = (diagnostic.line, diagnostic.column);
        let in_range = (first_line, first_column) <= at && at < (last_line, last_column);
        in_range
            && (self.codes.is_empty()
                || self
                    .codes
                    .iter()
                    .any(|code| diagnostic.rule.as_str().eq_ignore_ascii_case(code)))
    }

    const fn with_shell_coverage(mut self, coverage: Option<Coverage>) -> Self {
        match coverage {
            Some(Coverage::File) => self.scope = Scope::File,
            Some(Coverage::Lines(first, last)) => {
                self.covered_line = Some(first);
                self.last_covered_line = Some(last);
            }
            None => self.covered_line = None,
        }
        self
    }
}

pub fn directives(source: &str, language: Option<Language>) -> Vec<Directive> {
    let mut found = match language {
        Some(Language::Go) => go_directives(source),
        Some(language @ (Language::TypeScript | Language::Tsx | Language::JavaScript)) => {
            eslint_directives(source, language)
        }
        _ => Vec::new(),
    };
    found.extend(squawk_directives(source));
    found.extend(comment_directives(source));
    found
}

fn squawk_directives(source: &str) -> Vec<Directive> {
    squawk_ignores(source)
        .into_iter()
        .map(|ignore| Directive {
            line: ignore.line,
            text: ignore.text,
            scope: if ignore.whole_file {
                Scope::SquawkFile
            } else {
                Scope::SquawkLines
            },
            codes: ignore.rules,
            reason: ignore.reason,
            covered_line: Some(ignore.line),
            last_covered_line: Some(ignore.line.saturating_add(SQUAWK_NEXT_LINES)),
        })
        .collect()
}

fn go_directives(source: &str) -> Vec<Directive> {
    go_nolint_ranges(source)
        .into_iter()
        .map(|range| Directive {
            line: range.line,
            text: range.text,
            scope: Scope::GoNolint,
            codes: range.linters,
            reason: range.reason,
            covered_line: Some(range.line),
            last_covered_line: Some(range.last_line),
        })
        .collect()
}

fn eslint_directives(source: &str, language: Language) -> Vec<Directive> {
    eslint_disabled_ranges(source, language)
        .into_iter()
        .map(|range: DisabledRange| {
            let (last_line, last_column) = range.end.unwrap_or((usize::MAX, usize::MAX));
            Directive {
                line: range.line,
                text: range.text,
                scope: Scope::EslintRange {
                    first_column: range.start.1,
                    last_column,
                },
                codes: range.rules,
                reason: range.reason,
                covered_line: Some(range.start.0),
                last_covered_line: Some(last_line),
            }
        })
        .collect()
}

fn comment_directives(source: &str) -> Vec<Directive> {
    if !SYNTAXES.iter().any(|syntax| source.contains(syntax.marker)) {
        return Vec::new();
    }
    let lines: Vec<&str> = source.lines().collect();
    let mut shell_coverage: Option<HashMap<usize, Coverage>> = None;
    let mut found = Vec::new();
    for (index, text) in lines.iter().enumerate() {
        let Some((syntax, rest)) = directive_syntax(text) else {
            continue;
        };
        let (mut codes, reason) = codes_and_reason(rest);
        if syntax.scope == Scope::ShellStatement {
            for repeated in rest.split(REPEATED_SHELL_DISABLE).skip(1) {
                codes.extend(codes_and_reason(repeated).0);
            }
        }
        let covered_line = lines
            .iter()
            .enumerate()
            .skip(index.saturating_add(1))
            .find(|(_, later)| !is_blank_or_comment(later, syntax.comment))
            .map(|(later, _)| later.saturating_add(FIRST_LINE));
        let line = index.saturating_add(FIRST_LINE);
        let directive = Directive {
            line,
            text: text
                .trim_start()
                .strip_prefix(syntax.comment)
                .unwrap_or_default()
                .trim()
                .to_owned(),
            scope: syntax.scope,
            codes,
            reason,
            covered_line,
            last_covered_line: covered_line,
        };
        if syntax.scope == Scope::ShellStatement {
            let coverage = shell_coverage.get_or_insert_with(|| shell::directive_coverage(source));
            found.push(directive.with_shell_coverage(coverage.get(&line).copied()));
        } else {
            found.push(directive);
        }
    }
    found
}

pub fn split(source: &[u8], diagnostics: Vec<Diagnostic>) -> (Vec<Diagnostic>, Vec<Suppressed>) {
    if diagnostics.is_empty() {
        return (diagnostics, Vec::new());
    }
    let language = diagnostics
        .first()
        .and_then(|diagnostic| Language::from_path(Path::new(&diagnostic.path)));
    let found = directives(&String::from_utf8_lossy(source), language);
    if found.is_empty() {
        return (diagnostics, Vec::new());
    }
    let mut kept = Vec::new();
    let mut suppressed = Vec::new();
    for diagnostic in diagnostics {
        match found.iter().find(|directive| directive.covers(&diagnostic)) {
            Some(directive) => suppressed.push(Suppressed {
                reason: directive.reason.clone(),
                diagnostic,
            }),
            None => kept.push(diagnostic),
        }
    }
    (kept, suppressed)
}

fn directive_syntax(line: &str) -> Option<(Syntax, &str)> {
    let trimmed = line.trim_start();
    SYNTAXES.iter().find_map(|syntax| {
        let rest = trimmed.strip_prefix(syntax.comment)?.trim_start();
        rest.strip_prefix(syntax.marker).map(|rest| (*syntax, rest))
    })
}

fn codes_and_reason(rest: &str) -> (Vec<String>, String) {
    let mut codes = Vec::new();
    let mut remaining = rest.trim_start();
    loop {
        let length = remaining
            .find(|character: char| !is_code_character(character))
            .unwrap_or(remaining.len());
        let (code, after) = remaining.split_at(length);
        if code.is_empty() {
            break;
        }
        codes.push(code.to_owned());
        let Some(next) = after.trim_start().strip_prefix(CODE_SEPARATOR) else {
            remaining = after;
            break;
        };
        remaining = next.trim_start();
    }
    (
        codes,
        remaining.trim_start_matches(REASON_LEADERS).trim().to_owned(),
    )
}

const fn is_code_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '-' || character == '_'
}

fn is_blank_or_comment(line: &str, comment: &str) -> bool {
    let trimmed = line.trim();
    trimmed.is_empty() || trimmed.starts_with(comment)
}

#[cfg(test)]
mod tests;
