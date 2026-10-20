use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::config::Config;
use crate::diff::{ChangedLines, Changes};
use crate::i18n::{Cheat, Message};
use crate::language::Language;
use crate::rules::{Diagnostic, RuleId};

const CONFIG_NAMES: &[&str] = &[
    "kerbo.toml",
    "pyproject.toml",
    "ruff.toml",
    ".ruff.toml",
    "setup.cfg",
    ".coveragerc",
    "clippy.toml",
    ".clippy.toml",
    "Cargo.toml",
    "package.json",
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    ".golangci.yml",
    ".golangci.yaml",
];
const CONFIG_PREFIXES: &[&str] = &[
    "eslint.config.",
    ".eslintrc",
    "tsconfig",
    "checkstyle",
    "pmd",
    "vitest.config.",
    "jest.config.",
];
const COVERAGE_FILES: &[&str] = &["vitest.config.", "jest.config.", ".coveragerc"];
const COVERAGE_WORDS: &[&str] = &["coverage", "fail_under", "fail-under", "covered"];
const FLOOR_WORDS: &[&str] = &["minimum", "threshold"];
const CEILING_WORDS: &[&str] = &[
    "max",
    "complexity",
    "length",
    "lines",
    "statements",
    "param",
    "argument",
    "args",
    "depth",
    "nesting",
];
const FLOOR_PREFIX: &str = "min";
const CEILINGS_NAMED_MIN: &[&str] = &["complexity"];
const GENERIC_NAMES: &[&str] = &["value", "error", "warn", "property", "name"];
const LOOSENING_FLAGS: &[&str] = &["disable", "skip", "allow", "ignore"];
const NEGATION_PREFIX: &str = "no";
const TIGHTENING_FLAGS: &[&str] = &["strict", "enable", NEGATION_PREFIX];
const SEVERITIES: &[&str] = &["off", "warn", "error"];
const LINT_LEVELS: &[&str] = &["allow", "warn", "deny", "forbid"];
const PRESETS: &[&str] = &["strict", "recommended", "minimal"];
const BOOLEANS: &[&str] = &["false", "true"];
const PLACEHOLDER: &str = "#";
const KEY_CHARACTERS: &[char] = &['_', '-'];
const MEASURED_LANGUAGES: [Language; 5] = [
    Language::Go,
    Language::Python,
    Language::Java,
    Language::TypeScript,
    Language::Rust,
];
const FIRST_COLUMN: usize = 1;
const ALLOWED_OCCURRENCES: usize = 0;
const LOOSENED: usize = 1;

#[derive(Debug, Clone, PartialEq)]
enum Value {
    Number(f64),
    Word(String),
}

#[derive(Debug, Clone, PartialEq)]
struct Setting {
    context: String,
    key: String,
    value: Value,
    shown: String,
}

#[derive(Debug)]
struct Loosening {
    line: usize,
    old: Setting,
    new: Setting,
}

pub fn loosened_settings(changes: &ChangedLines, config: &Config, project_root: &Path) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for (path, file_changes) in changes.files() {
        let Some(relative) = relative_config_path(path, project_root, config) else {
            continue;
        };
        let (Ok(current), Some(previous)) = (fs::read_to_string(path), changes.previous_version(path)) else {
            continue;
        };
        let mut occurrences: HashMap<String, usize> = HashMap::new();
        for loosening in loosened_lines(&current, &previous, file_changes, &relative) {
            let occurrence = occurrences.entry(loosening.new.key.clone()).or_default();
            *occurrence = occurrence.saturating_add(1);
            diagnostics.push(diagnostic(path, &relative, &loosening, *occurrence));
        }
    }
    diagnostics
}

fn relative_config_path(path: &Path, project_root: &Path, config: &Config) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    let is_config =
        CONFIG_NAMES.contains(&name) || CONFIG_PREFIXES.iter().any(|prefix| name.starts_with(prefix));
    let relative = path
        .strip_prefix(project_root)
        .ok()?
        .to_string_lossy()
        .replace('\\', "/");
    let reported = MEASURED_LANGUAGES.iter().any(|&language| {
        config
            .limits_for(Path::new(&relative), language, false)
            .get(RuleId::SuppressionAdded)
            .is_some()
    });
    (is_config && reported && !config.is_ignored(Path::new(&relative))).then_some(relative)
}

fn loosened_lines(current: &str, previous: &str, changes: &Changes, relative: &str) -> Vec<Loosening> {
    let current_lines: Vec<&str> = current.lines().collect();
    let previous_settings: Vec<Setting> = contextual_settings(previous)
        .into_iter()
        .filter(|(_, raw, _)| !current_lines.contains(raw))
        .map(|(_, _, setting)| setting)
        .collect();
    let coverage_file = Path::new(relative)
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| COVERAGE_FILES.iter().any(|prefix| name.starts_with(prefix)));
    contextual_settings(current)
        .into_iter()
        .filter(|&(index, _, _)| changes.adds_line(index.saturating_add(1)))
        .filter_map(|(index, _, new)| {
            let old = previous_settings
                .iter()
                .find(|old| old.context == new.context && old.key == new.key && old.value != new.value)?;
            is_looser(old, &new, coverage_file).then(|| Loosening {
                line: index.saturating_add(1),
                old: old.clone(),
                new,
            })
        })
        .collect()
}

fn contextual_settings(text: &str) -> Vec<(usize, &str, Setting)> {
    let lines: Vec<&str> = text.lines().collect();
    lines
        .iter()
        .enumerate()
        .flat_map(|(index, &line)| {
            let context = parent_line(lines.get(..index).unwrap_or_default(), line);
            settings_of(line).into_iter().map(move |setting| {
                (
                    index,
                    line,
                    Setting {
                        context: context.to_owned(),
                        ..setting
                    },
                )
            })
        })
        .collect()
}

fn parent_line<'text>(before: &[&'text str], line: &str) -> &'text str {
    let depth = indentation(line);
    before
        .iter()
        .rev()
        .find(|candidate| !candidate.trim().is_empty() && indentation(candidate) < depth)
        .map_or("", |parent| parent.trim())
}

fn indentation(line: &str) -> usize {
    line.chars()
        .take_while(|character| character.is_whitespace())
        .count()
}

fn settings_of(line: &str) -> Vec<Setting> {
    let trimmed = line.trim();
    let mut found = Vec::new();
    if let Some((start, end)) = last_number(trimmed)
        && let Some(number) = trimmed.get(start..end).and_then(|text| text.parse().ok())
    {
        found.push(setting(trimmed, start, end, Value::Number(number)));
    }
    for (start, word) in words(trimmed) {
        let known = [SEVERITIES, LINT_LEVELS, PRESETS, BOOLEANS]
            .iter()
            .any(|scale| scale.contains(&word));
        if known {
            let end = start.saturating_add(word.len());
            found.push(setting(trimmed, start, end, Value::Word(word.to_owned())));
        }
    }
    found
}

fn setting(line: &str, start: usize, end: usize, value: Value) -> Setting {
    let key = format!(
        "{}{PLACEHOLDER}{}",
        line.get(..start).unwrap_or_default(),
        line.get(end..).unwrap_or_default()
    );
    Setting {
        context: String::new(),
        key: key.split_whitespace().collect::<Vec<_>>().join(" "),
        value,
        shown: line.to_owned(),
    }
}

fn last_number(line: &str) -> Option<(usize, usize)> {
    let bytes = line.as_bytes();
    let end = bytes.iter().rposition(u8::is_ascii_digit)?.saturating_add(1);
    let start = bytes
        .get(..end)?
        .iter()
        .rposition(|&byte| !byte.is_ascii_digit() && byte != b'.')
        .map_or(0, |position| position.saturating_add(1));
    let touches_word = start
        .checked_sub(1)
        .and_then(|before| bytes.get(before))
        .is_some_and(|byte| byte.is_ascii_alphabetic() || *byte == b'_');
    (!touches_word).then_some((start, end))
}

fn words(line: &str) -> Vec<(usize, &str)> {
    let mut found = Vec::new();
    let mut start = None;
    for (index, character) in line.char_indices().chain(std::iter::once((line.len(), ' '))) {
        let in_word = character.is_ascii_alphabetic();
        match (start, in_word) {
            (None, true) => start = Some(index),
            (Some(first), false) => {
                found.extend(line.get(first..index).map(|word| (first, word)));
                start = None;
            }
            _ => {}
        }
    }
    found
}

fn is_looser(old: &Setting, new: &Setting, coverage_file: bool) -> bool {
    let key = old.key.to_ascii_lowercase();
    match (&old.value, &new.value) {
        (Value::Number(before), Value::Number(after)) => {
            let name = setting_name(&key);
            let floor_word = mentions(&key, FLOOR_WORDS) && !mentions(name, CEILING_WORDS);
            if coverage_file || mentions(&key, COVERAGE_WORDS) || floor_word || is_floor(name) {
                after < before
            } else if mentions(name, CEILING_WORDS) || mentions(&key, CEILING_WORDS) {
                after > before
            } else {
                false
            }
        }
        (Value::Word(before), Value::Word(after)) => looser_word(&old.key, before, after),
        _ => false,
    }
}

fn looser_word(key: &str, before: &str, after: &str) -> bool {
    let rank = |scale: &[&str], word: &str| scale.iter().position(|entry| *entry == word);
    if let (Some(old), Some(new)) = (rank(SEVERITIES, before), rank(SEVERITIES, after)) {
        return new < old;
    }
    if let (Some(old), Some(new)) = (rank(LINT_LEVELS, before), rank(LINT_LEVELS, after)) {
        return new < old;
    }
    if let (Some(old), Some(new)) = (rank(PRESETS, before), rank(PRESETS, after)) {
        return new > old;
    }
    let (Some(old), Some(new)) = (rank(BOOLEANS, before), rank(BOOLEANS, after)) else {
        return false;
    };
    if mentions(&key.to_ascii_lowercase(), LOOSENING_FLAGS) {
        new > old
    } else if TIGHTENING_FLAGS.iter().any(|flag| names_flag(key, flag)) {
        new < old
    } else {
        false
    }
}

fn setting_name(key: &str) -> &str {
    let before_value = key.split(PLACEHOLDER).next().unwrap_or_default();
    before_value
        .split(|character: char| !character.is_ascii_alphanumeric() && !KEY_CHARACTERS.contains(&character))
        .rfind(|part| !part.is_empty() && !GENERIC_NAMES.contains(part))
        .unwrap_or_default()
}

fn is_floor(name: &str) -> bool {
    name.starts_with(FLOOR_PREFIX) && !mentions(name, CEILINGS_NAMED_MIN)
}

fn mentions(key: &str, words: &[&str]) -> bool {
    words.iter().any(|word| key.contains(word))
}

fn names_flag(key: &str, flag: &str) -> bool {
    key.split(|character: char| !character.is_ascii_alphanumeric() && !KEY_CHARACTERS.contains(&character))
        .any(|part| {
            part.strip_prefix(flag).is_some_and(|rest| {
                flag != NEGATION_PREFIX || rest.is_empty() || rest.starts_with(char::is_uppercase)
            })
        })
}

fn diagnostic(path: &Path, relative: &str, loosening: &Loosening, occurrence: usize) -> Diagnostic {
    let Loosening { line, old, new } = loosening;
    let line = *line;
    Diagnostic {
        path: relative.to_owned(),
        line,
        column: FIRST_COLUMN,
        end_line: line,
        rule: RuleId::SuppressionAdded,
        message: Message::Cheat(Cheat::ThresholdLoosened {
            old: old.shown.clone(),
            new: new.shown.clone(),
        }),
        value: LOOSENED,
        limit: ALLOWED_OCCURRENCES,
        fingerprint: format!("suppression-added:{relative}:{}#{occurrence}", new.key),
        relative_path: relative.to_owned(),
        absolute_path: path.to_path_buf(),
    }
}

#[cfg(test)]
mod tests;
