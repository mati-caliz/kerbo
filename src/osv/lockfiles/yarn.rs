use std::collections::HashSet;

use super::{Dependency, numbered_lines};
use crate::osv::Ecosystem;

const COMMENT: char = '#';
const HEADER_END: char = ':';
const SPEC_SEPARATOR: char = ',';
const QUOTES: &[char] = &['\'', '"'];
const SCOPE_MARK: char = '@';
const VERSION_PREFIXES: [&str; 2] = ["version:", "version "];
const LOCAL_PROTOCOLS: [&str; 4] = ["@workspace:", "@link:", "@portal:", "@file:"];

pub fn lockfile(text: &str) -> Vec<Dependency> {
    let mut found = Vec::new();
    let mut seen = HashSet::new();
    let mut current: Option<(String, usize)> = None;
    for (number, line) in numbered_lines(text) {
        if line.trim().is_empty() || line.starts_with(COMMENT) {
            continue;
        }
        if !line.starts_with(' ') {
            current = line
                .trim_end()
                .strip_suffix(HEADER_END)
                .and_then(package_name)
                .map(|name| (name, number));
            continue;
        }
        let Some(version) = version_of(line) else {
            continue;
        };
        if let Some((name, header)) = current.take()
            && seen.insert((name.clone(), version.clone()))
        {
            found.push(Dependency {
                ecosystem: Ecosystem::Npm,
                name,
                version,
                line: header,
            });
        }
    }
    found
}

fn package_name(header: &str) -> Option<String> {
    let first = header.split(SPEC_SEPARATOR).next()?.trim().trim_matches(QUOTES);
    if LOCAL_PROTOCOLS.iter().any(|protocol| first.contains(protocol)) {
        return None;
    }
    let separator = first
        .char_indices()
        .skip(1)
        .find(|&(_, character)| character == SCOPE_MARK)
        .map(|(index, _)| index)?;
    first.get(..separator).map(str::to_owned)
}

fn version_of(line: &str) -> Option<String> {
    let trimmed = line.trim();
    VERSION_PREFIXES
        .iter()
        .find_map(|prefix| trimmed.strip_prefix(prefix))
        .map(|version| version.trim().trim_matches(QUOTES).to_owned())
}
