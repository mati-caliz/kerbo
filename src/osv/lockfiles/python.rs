use serde_json::Value;

use super::{Dependency, first_line_containing, numbered_lines};
use crate::osv::Ecosystem;

const COMMENT: char = '#';
const OPTION_PREFIX: char = '-';
const CONTINUATION: char = '\\';
const MARKER_SEPARATOR: char = ';';
const NAME_ENDS: &[char] = &['[', '=', '<', '>', '!', '~', ' ', '(', '@'];
const EXACT_OPERATORS: [&str; 2] = ["===", "=="];
const CLAUSE_SEPARATOR: char = ',';
const WILDCARD: char = '*';
const URL_MARK: &str = "://";
const EXTRAS_START: char = '[';
const EXTRAS_END: char = ']';
const PIPFILE_PIN: &str = "==";
const PIPFILE_SECTIONS: [&str; 2] = ["default", "develop"];
const VERSION_KEY: &str = "version";

pub fn requirements(text: &str) -> Vec<Dependency> {
    numbered_lines(text)
        .filter_map(|(number, line)| {
            let (name, version) = pinned(line)?;
            Some(Dependency {
                ecosystem: Ecosystem::PyPi,
                name,
                version,
                line: number,
            })
        })
        .collect()
}

fn pinned(line: &str) -> Option<(String, String)> {
    let requirement = line
        .split(COMMENT)
        .next()?
        .trim()
        .trim_end_matches(CONTINUATION)
        .split(MARKER_SEPARATOR)
        .next()?
        .trim();
    if requirement.is_empty() || requirement.starts_with(OPTION_PREFIX) || requirement.contains(URL_MARK) {
        return None;
    }
    let name_end = requirement.find(NAME_ENDS).unwrap_or(requirement.len());
    let (name, rest) = requirement.split_at(name_end);
    let rest = rest.trim_start();
    let specifier = if rest.starts_with(EXTRAS_START) {
        rest.split_once(EXTRAS_END).map_or("", |(_, after)| after)
    } else {
        rest
    }
    .trim();
    if specifier.contains(CLAUSE_SEPARATOR) {
        return None;
    }
    let version = EXACT_OPERATORS
        .iter()
        .find_map(|operator| specifier.strip_prefix(operator))?
        .trim();
    if version.contains(WILDCARD) || name.is_empty() {
        return None;
    }
    Some((name.to_lowercase(), version.to_owned()))
}

pub fn pipfile_lock(text: &str) -> Vec<Dependency> {
    let Ok(document) = serde_json::from_str::<Value>(text) else {
        return Vec::new();
    };
    PIPFILE_SECTIONS
        .iter()
        .filter_map(|section| document.get(section).and_then(Value::as_object))
        .flatten()
        .filter_map(|(name, entry)| {
            let version = entry
                .get(VERSION_KEY)
                .and_then(Value::as_str)?
                .strip_prefix(PIPFILE_PIN)?;
            Some(Dependency {
                ecosystem: Ecosystem::PyPi,
                name: name.to_lowercase(),
                version: version.to_owned(),
                line: first_line_containing(text, &format!("\"{name}\": {{")),
            })
        })
        .collect()
}
