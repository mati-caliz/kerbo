use std::collections::{HashMap, HashSet};

use serde_json::{Map, Value};

use super::{Dependency, numbered_lines};
use crate::osv::Ecosystem;

const PACKAGES_KEY: &str = "packages";
const DEPENDENCIES_KEY: &str = "dependencies";
const NAME_KEY: &str = "name";
const VERSION_KEY: &str = "version";
const LINK_KEY: &str = "link";
const BUNDLED_KEYS: [&str; 2] = ["inBundle", "bundled"];
const MODULES_DIRECTORY: &str = "node_modules/";
const QUOTE: char = '"';
const OBJECT_KEY_END: &str = "\": {";
const FIRST_LINE: usize = 1;

struct KeyLines<'text> {
    first: HashMap<&'text str, usize>,
}

impl<'text> KeyLines<'text> {
    fn new(text: &'text str) -> Self {
        let mut first = HashMap::new();
        for (number, line) in numbered_lines(text) {
            let key = line
                .trim()
                .strip_prefix(QUOTE)
                .and_then(|rest| rest.strip_suffix(OBJECT_KEY_END));
            if let Some(key) = key {
                first.entry(key).or_insert(number);
            }
        }
        Self { first }
    }

    fn line(&self, key: &str) -> usize {
        self.first.get(key).copied().unwrap_or(FIRST_LINE)
    }
}

pub fn package_lock(text: &str) -> Vec<Dependency> {
    let Ok(document) = serde_json::from_str::<Value>(text) else {
        return Vec::new();
    };
    let lines = KeyLines::new(text);
    let mut found = Vec::new();
    if let Some(packages) = document.get(PACKAGES_KEY).and_then(Value::as_object) {
        found.extend(
            packages
                .iter()
                .filter_map(|(key, entry)| modern_entry(&lines, key, entry)),
        );
    } else if let Some(dependencies) = document.get(DEPENDENCIES_KEY).and_then(Value::as_object) {
        legacy_entries(&lines, dependencies, &mut found);
    }
    let mut seen = HashSet::new();
    found.retain(|dependency| seen.insert((dependency.name.clone(), dependency.version.clone())));
    found
}

fn modern_entry(lines: &KeyLines<'_>, key: &str, entry: &Value) -> Option<Dependency> {
    let (_, installed) = key.rsplit_once(MODULES_DIRECTORY)?;
    if is_flagged(entry, LINK_KEY) || is_bundled(entry) {
        return None;
    }
    let name = entry.get(NAME_KEY).and_then(Value::as_str).unwrap_or(installed);
    Some(Dependency {
        ecosystem: Ecosystem::Npm,
        name: name.to_owned(),
        version: registry_version(entry)?,
        line: lines.line(key),
    })
}

fn legacy_entries(lines: &KeyLines<'_>, dependencies: &Map<String, Value>, found: &mut Vec<Dependency>) {
    for (name, entry) in dependencies {
        if let Some(version) = registry_version(entry).filter(|_| !is_bundled(entry)) {
            found.push(Dependency {
                ecosystem: Ecosystem::Npm,
                name: name.clone(),
                version,
                line: lines.line(name),
            });
        }
        if let Some(nested) = entry.get(DEPENDENCIES_KEY).and_then(Value::as_object) {
            legacy_entries(lines, nested, found);
        }
    }
}

fn is_flagged(entry: &Value, key: &str) -> bool {
    entry.get(key).and_then(Value::as_bool).unwrap_or_default()
}

fn is_bundled(entry: &Value) -> bool {
    BUNDLED_KEYS.iter().any(|key| is_flagged(entry, key))
}

fn registry_version(entry: &Value) -> Option<String> {
    entry
        .get(VERSION_KEY)
        .and_then(Value::as_str)
        .filter(|version| version.starts_with(|first: char| first.is_ascii_digit()))
        .map(str::to_owned)
}
