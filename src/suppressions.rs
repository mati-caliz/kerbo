use crate::rules::Diagnostic;

const CODE_SEPARATOR: char = ',';
const REASON_LEADERS: &[char] = &[' ', '\t', '#', '-', ':', '—'];
const FIRST_LINE: usize = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    NextLine,
    File,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Syntax {
    pub comment: &'static str,
    pub marker: &'static str,
    pub scope: Scope,
}

pub const SYNTAXES: [Syntax; 5] = [
    Syntax {
        comment: "--",
        marker: "squawk-ignore-file",
        scope: Scope::File,
    },
    Syntax {
        comment: "--",
        marker: "squawk-ignore",
        scope: Scope::NextLine,
    },
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
        scope: Scope::NextLine,
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
}

#[derive(Debug, Clone)]
pub struct Suppressed {
    pub diagnostic: Diagnostic,
    pub reason: String,
}

impl Directive {
    fn covers(&self, diagnostic: &Diagnostic) -> bool {
        let in_scope = match self.scope {
            Scope::File => true,
            Scope::NextLine => self.covered_line == Some(diagnostic.line),
        };
        in_scope
            && self
                .codes
                .iter()
                .any(|code| diagnostic.rule.as_str().eq_ignore_ascii_case(code))
    }
}

pub fn directives(source: &str) -> Vec<Directive> {
    if !SYNTAXES.iter().any(|syntax| source.contains(syntax.marker)) {
        return Vec::new();
    }
    let lines: Vec<&str> = source.lines().collect();
    let mut found = Vec::new();
    for (index, text) in lines.iter().enumerate() {
        let Some((syntax, rest)) = directive_syntax(text) else {
            continue;
        };
        let (codes, reason) = codes_and_reason(rest);
        let covered_line = lines
            .iter()
            .enumerate()
            .skip(index.saturating_add(1))
            .find(|(_, later)| !is_blank_or_comment(later, syntax.comment))
            .map(|(later, _)| later.saturating_add(FIRST_LINE));
        found.push(Directive {
            line: index.saturating_add(FIRST_LINE),
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
        });
    }
    found
}

pub fn split(source: &[u8], diagnostics: Vec<Diagnostic>) -> (Vec<Diagnostic>, Vec<Suppressed>) {
    if diagnostics.is_empty() {
        return (diagnostics, Vec::new());
    }
    let found = directives(&String::from_utf8_lossy(source));
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
