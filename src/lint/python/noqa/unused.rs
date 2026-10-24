use std::collections::{HashMap, HashSet};

use super::ruff_codes::{DEFAULT_CODES, RUFF_CODES};
use super::{Directive, Placed, Suppressions, redirected};
use crate::lint::python::Report;
use crate::rules::PythonRule;

const SELF_CODE: &str = "RUF100";
const UNUSED: &str = "unused";
const NON_ENABLED: &str = "non-enabled";
const DUPLICATED: &str = "duplicated";
const GROUP_SEPARATOR: &str = "; ";
const CODE_SEPARATOR: &str = ", ";

#[derive(Debug, Default)]
struct Unused<'code> {
    unmatched: Vec<&'code str>,
    disabled: Vec<&'code str>,
    duplicated: Vec<&'code str>,
}

impl Unused<'_> {
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
    enabled: &dyn Fn(PythonRule) -> bool,
    complete: bool,
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
        .filter_map(|(placed, used)| unused_report(placed, used, enabled, complete))
        .collect()
}
fn unused_report(
    placed: &Placed,
    used: &HashSet<String>,
    enabled: &dyn Fn(PythonRule) -> bool,
    complete: bool,
) -> Option<Report> {
    let subject = match &placed.directive {
        Directive::All => (complete && used.is_empty()).then(String::new)?,
        Directive::Codes(codes) => classified(codes, used, enabled, complete)?.subject()?,
    };
    Some(Report::at_point(PythonRule::RUF100, placed.start, &subject))
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
