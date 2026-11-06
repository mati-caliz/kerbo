use std::fs;
use std::path::Path;

use super::directives::disables;
use crate::rules::{Diagnostic, RuleId};
use crate::suppressions::Suppressed;

pub const RC_FILE_NAMES: [&str; 2] = [".shellcheckrc", "shellcheckrc"];
const REPOSITORY_MARKER: &str = ".git";
const COMMENT_START: char = '#';
const KEY_VALUE_SEPARATOR: char = '=';
const CODE_SEPARATOR: char = ',';
const DISABLE_KEY: &str = "disable";
const SHELL_KEY: &str = "shell";
const QUOTES: &[char] = &['\'', '"'];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RcFile {
    pub name: &'static str,
    pub disabled: Vec<String>,
    pub shell: Option<String>,
}

// Como ShellCheck, gana el primero que aparece subiendo; a diferencia de ShellCheck no se sale del
// repositorio, para que el resultado no dependa del `~/.shellcheckrc` de cada máquina.
pub fn find(source_path: &Path) -> Option<RcFile> {
    for directory in source_path.ancestors().skip(1) {
        let found = read_in(directory);
        if found.is_some() || directory.join(REPOSITORY_MARKER).exists() {
            return found;
        }
    }
    None
}

fn read_in(directory: &Path) -> Option<RcFile> {
    RC_FILE_NAMES.into_iter().find_map(|name| {
        let text = fs::read_to_string(directory.join(name)).ok()?;
        Some(parse(name, &text))
    })
}

fn parse(name: &'static str, text: &str) -> RcFile {
    let mut rc_file = RcFile {
        name,
        ..RcFile::default()
    };
    for pair in text.lines().flat_map(active_pairs) {
        match pair {
            (DISABLE_KEY, codes) => rc_file
                .disabled
                .extend(codes.split(CODE_SEPARATOR).map(str::to_owned)),
            (SHELL_KEY, shell) if rc_file.shell.is_none() => {
                rc_file.shell = Some(shell.trim_matches(QUOTES).to_owned());
            }
            _ => {}
        }
    }
    rc_file
}

fn active_pairs(line: &str) -> impl Iterator<Item = (&str, &str)> {
    let active = line.split(COMMENT_START).next().unwrap_or_default();
    active
        .split_whitespace()
        .filter_map(|pair| pair.split_once(KEY_VALUE_SEPARATOR))
}

pub fn split_disabled_by_rc(
    source_path: &Path,
    diagnostics: Vec<Diagnostic>,
) -> (Vec<Diagnostic>, Vec<Suppressed>) {
    let has_shell_rule = diagnostics
        .iter()
        .any(|diagnostic| matches!(diagnostic.rule, RuleId::Shell(_)));
    let rc_file = has_shell_rule.then(|| find(source_path)).flatten();
    let Some(rc_file) = rc_file else {
        return (diagnostics, Vec::new());
    };
    let (disabled, kept): (Vec<Diagnostic>, Vec<Diagnostic>) = diagnostics
        .into_iter()
        .partition(|diagnostic| rc_file_disables(&rc_file, diagnostic.rule));
    let suppressed = disabled
        .into_iter()
        .map(|diagnostic| Suppressed {
            diagnostic,
            reason: rc_file.name.to_owned(),
        })
        .collect();
    (kept, suppressed)
}

fn rc_file_disables(rc_file: &RcFile, rule: RuleId) -> bool {
    matches!(rule, RuleId::Shell(_)) && rc_file.disabled.iter().any(|code| disables(code, rule.as_str()))
}

pub fn disables_something_new(line: &str, previous_text: &str) -> bool {
    let current = parse("", line).disabled;
    let previous = parse("", previous_text).disabled;
    current.iter().any(|code| !previous.contains(code))
}

#[cfg(test)]
mod tests;
