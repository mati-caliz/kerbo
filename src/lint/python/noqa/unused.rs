use std::collections::{HashMap, HashSet};

use super::ruff_codes::{DEFAULT_CODES, RUFF_CODES};
use super::{Directive, Placed, Suppressions, redirected};
use crate::fix::{Fix, TextEdit};
use crate::lint::python::Report;
use crate::lint::python::fix_edits::{delete_between, point_of};
use crate::rules::PythonRule;

const SELF_CODE: &str = "RUF100";
const UNUSED: &str = "unused";
const NON_ENABLED: &str = "non-enabled";
const DUPLICATED: &str = "duplicated";
const GROUP_SEPARATOR: &str = "; ";
const CODE_SEPARATOR: &str = ", ";
const LINE_PREFIX: &str = "# noqa: ";
const RUFF_FILE_PREFIX: &str = "# ruff: noqa: ";
const FLAKE8_FILE_PREFIX: &str = "# flake8: noqa: ";
const LINE_FEED: u8 = b'\n';
const CARRIAGE_RETURN: u8 = b'\r';
const HASH: u8 = b'#';
const WHITESPACE: &[u8] = b" \t\x0c";

#[derive(Debug, Default)]
struct Unused<'code> {
    unmatched: Vec<&'code str>,
    disabled: Vec<&'code str>,
    duplicated: Vec<&'code str>,
}

impl Unused<'_> {
    fn contains(&self, code: &str) -> bool {
        [&self.unmatched, &self.disabled]
            .into_iter()
            .any(|codes| codes.contains(&code))
    }

    fn subject(&self) -> Option<String> {
        let groups: Vec<String> = [
            (UNUSED, &self.unmatched),
            (NON_ENABLED, &self.disabled),
            (DUPLICATED, &self.duplicated),
        ]
        .into_iter()
        .filter(|(_, codes)| !codes.is_empty())
        .map(|(label, codes)| format!("{label}: {}", codes.join(CODE_SEPARATOR)))
        .collect();
        (!groups.is_empty()).then(|| groups.join(GROUP_SEPARATOR))
    }
}

// RUF100: un `noqa` que no silencia nada, o códigos de más en él. Como el `check_noqa` de ruff:
// cada aviso cuenta para el primer `noqa` que lo silencia, un código de una regla apagada es
// "non-enabled" y uno que ruff no conoce se deja (es otra regla). Si kerbo no implementa una
// regla del conjunto por defecto de ruff, no puede saber si se usa y no la marca. Sin el árbol
// completo (`complete` en falso) faltan avisos, así que sólo marca lo que no depende de ellos.
pub fn unused_directives(
    suppressions: &Suppressions,
    reports: &[Report],
    check: &Check<'_, '_>,
) -> Vec<Report> {
    if suppressions.exempts(SELF_CODE) {
        return Vec::new();
    }
    let mut matches: HashMap<usize, HashSet<String>> = HashMap::new();
    let mut reported: HashSet<String> = HashSet::new();
    for report in reports {
        let code = report.rule.as_str().to_uppercase();
        reported.insert(code.clone());
        if suppressions.exempts(&code) {
            continue;
        }
        if let Some(row) = suppressions.silencing_row(report, &code) {
            matches.entry(row).or_default().insert(code);
        }
    }
    let empty = HashSet::new();
    let lines = suppressions
        .lines
        .iter()
        .map(|(row, placed)| (placed, matches.get(row).unwrap_or(&empty)));
    let files = suppressions.files.iter().map(|placed| (placed, &reported));
    lines
        .chain(files)
        .filter_map(|(placed, used)| unused_report(placed, used, check))
        .collect()
}
fn unused_report(placed: &Placed, used: &HashSet<String>, check: &Check<'_, '_>) -> Option<Report> {
    let (subject, kept) = match &placed.directive {
        Directive::All => ((check.complete && used.is_empty()).then(String::new)?, Vec::new()),
        Directive::Codes(codes) => {
            let unused = classified(codes, used, check.enabled, check.complete)?;
            let kept: Vec<&str> = codes
                .iter()
                .map(String::as_str)
                .filter(|code| !unused.contains(code))
                .fold(Vec::new(), |mut kept, code| {
                    if !kept.contains(&code) {
                        kept.push(code);
                    }
                    kept
                });
            (unused.subject()?, kept)
        }
    };
    let fix = if kept.is_empty() {
        delete_comment(placed.start_byte, placed.end_byte, check.source)
    } else {
        let prefix = match (placed.file, placed.flake8) {
            (true, true) => FLAKE8_FILE_PREFIX,
            (true, false) => RUFF_FILE_PREFIX,
            _ => LINE_PREFIX,
        };
        let replacement = format!("{prefix}{}", kept.join(CODE_SEPARATOR));
        Fix::from_edits(
            check.source,
            vec![TextEdit {
                start: point_of(check.source, placed.start_byte),
                end: point_of(check.source, placed.end_byte),
                replacement,
            }],
        )
    };
    Some(Report::at_point(PythonRule::RUF100, placed.start, &subject).with_fix(Some(fix)))
}

pub struct Check<'enabled, 'source> {
    pub enabled: &'enabled dyn Fn(PythonRule) -> bool,
    pub complete: bool,
    pub source: &'source [u8],
}

// El `delete_comment` de ruff: la línea entera si es sólo el comentario; hasta el final de la línea
// si no sigue nada; sólo la directiva si después viene otro comentario.
fn delete_comment(start: usize, end: usize, source: &[u8]) -> Fix {
    let line_start = source
        .get(..start)
        .and_then(|before| before.iter().rposition(|byte| *byte == LINE_FEED))
        .map_or(0, |newline| newline.saturating_add(1));
    let full_line_end = source
        .get(end..)
        .and_then(|rest| rest.iter().position(|byte| *byte == LINE_FEED))
        .map_or(source.len(), |newline| {
            end.saturating_add(newline).saturating_add(1)
        });
    let line_end = if source.get(full_line_end.saturating_sub(1)) == Some(&LINE_FEED) {
        let without_feed = full_line_end.saturating_sub(1);
        if source.get(without_feed.saturating_sub(1)) == Some(&CARRIAGE_RETURN) {
            without_feed.saturating_sub(1)
        } else {
            without_feed
        }
    } else {
        full_line_end
    };
    let before = source.get(line_start..start).unwrap_or_default();
    let leading = before.len().saturating_sub(trimmed_end(before).len());
    let after = source.get(end..line_end).unwrap_or_default();
    let trailing = after.len().saturating_sub(trimmed_start(after).len());
    let (from, to) =
        if start.saturating_sub(leading) == line_start && end.saturating_add(trailing) == line_end {
            (line_start, full_line_end)
        } else if end.saturating_add(trailing) == line_end {
            (start.saturating_sub(leading), line_end)
        } else if source.get(end.saturating_add(trailing)) == Some(&HASH) {
            (start, end.saturating_add(trailing))
        } else {
            (start.saturating_sub(leading), line_end)
        };
    delete_between(from, to, source)
}

fn trimmed_end(bytes: &[u8]) -> &[u8] {
    let kept = bytes
        .iter()
        .rposition(|byte| !WHITESPACE.contains(byte))
        .map_or(0, |index| index.saturating_add(1));
    bytes.get(..kept).unwrap_or_default()
}

fn trimmed_start(bytes: &[u8]) -> &[u8] {
    let skipped = bytes
        .iter()
        .position(|byte| !WHITESPACE.contains(byte))
        .unwrap_or(bytes.len());
    bytes.get(skipped..).unwrap_or_default()
}

fn classified<'code>(
    codes: &'code [String],
    used: &HashSet<String>,
    enabled: &dyn Fn(PythonRule) -> bool,
    complete: bool,
) -> Option<Unused<'code>> {
    let mut unused = Unused::default();
    let mut seen = HashSet::new();
    for original in codes {
        let code = redirected(original);
        if !seen.insert(original.as_str()) {
            unused.duplicated.push(original);
            continue;
        }
        if code == SELF_CODE {
            return None;
        }
        if used.contains(code) || RUFF_CODES.binary_search(&code).is_err() {
            continue;
        }
        let rule = rule_for(code);
        let is_enabled = rule.is_some_and(enabled);
        let unknown_to_kerbo = rule.is_none() && DEFAULT_CODES.binary_search(&code).is_ok();
        if is_enabled {
            if complete {
                unused.unmatched.push(original);
            }
        } else if !unknown_to_kerbo {
            unused.disabled.push(original);
        }
    }
    Some(unused)
}

fn rule_for(code: &str) -> Option<PythonRule> {
    PythonRule::ALL
        .into_iter()
        .find(|rule| rule.as_str().eq_ignore_ascii_case(code))
}
