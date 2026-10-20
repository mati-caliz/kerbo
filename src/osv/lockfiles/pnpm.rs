use std::collections::HashSet;

use super::{Dependency, numbered_lines};
use crate::osv::Ecosystem;

const VERSION_KEY: &str = "lockfileVersion:";
const PACKAGES_SECTION: &str = "packages:";
const ENTRY_INDENT: &str = "  ";
const LEGACY_MAJOR: char = '5';
const QUOTES: &[char] = &['\'', '"'];
const PEERS_START: char = '(';
const LEGACY_PEERS_START: char = '_';
const SCOPE_MARK: char = '@';
const PATH_SEPARATOR: char = '/';
const KEY_END: char = ':';

pub fn lockfile(text: &str) -> Vec<Dependency> {
    let legacy = numbered_lines(text)
        .find_map(|(_, line)| line.strip_prefix(VERSION_KEY))
        .is_some_and(|version| version.trim().trim_matches(QUOTES).starts_with(LEGACY_MAJOR));
    let mut inside = false;
    let mut seen = HashSet::new();
    let mut found = Vec::new();
    for (number, line) in numbered_lines(text) {
        if !line.starts_with(' ') && !line.trim().is_empty() {
            inside = line.trim_end() == PACKAGES_SECTION;
            continue;
        }
        let Some(key) = entry_key(line).filter(|_| inside) else {
            continue;
        };
        let Some((name, version)) = split_key(key, legacy) else {
            continue;
        };
        if version.starts_with(|first: char| first.is_ascii_digit())
            && seen.insert((name.clone(), version.clone()))
        {
            found.push(Dependency {
                ecosystem: Ecosystem::Npm,
                name,
                version,
                line: number,
            });
        }
    }
    found
}

fn entry_key(line: &str) -> Option<&str> {
    let rest = line.strip_prefix(ENTRY_INDENT)?;
    if rest.starts_with(' ') {
        return None;
    }
    let key = rest.trim_end().strip_suffix(KEY_END)?;
    Some(key.trim_matches(QUOTES).trim_start_matches(PATH_SEPARATOR))
}

fn split_key(key: &str, legacy: bool) -> Option<(String, String)> {
    if legacy {
        let (name, version) = key.rsplit_once(PATH_SEPARATOR)?;
        let version = version.split(LEGACY_PEERS_START).next().unwrap_or(version);
        return Some((name.to_owned(), version.to_owned()));
    }
    let without_peers = key.split(PEERS_START).next().unwrap_or(key);
    let separator = without_peers
        .char_indices()
        .skip(1)
        .filter(|&(_, character)| character == SCOPE_MARK)
        .map(|(index, _)| index)
        .last()?;
    let (name, version) = without_peers.split_at(separator);
    Some((name.to_owned(), version.trim_start_matches(SCOPE_MARK).to_owned()))
}
