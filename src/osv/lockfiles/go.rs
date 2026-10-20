use std::collections::{HashMap, HashSet};

use super::{Dependency, numbered_lines};
use crate::osv::Ecosystem;

const REQUIRE: &str = "require";
const REPLACE: &str = "replace";
const BLOCK_OPEN: &str = "(";
const BLOCK_CLOSE: &str = ")";
const COMMENT: &str = "//";
const ARROW: &str = "=>";
const VERSION_PREFIX: char = 'v';
const MODULE_FILE_SUFFIX: &str = "/go.mod";
const LOCAL_PATH_PREFIXES: [&str; 2] = ["./", "../"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Directive {
    Require,
    Replace,
    Other,
}

struct Replacement {
    version: Option<String>,
    target: Option<(String, String)>,
}

#[derive(Default)]
struct ModuleFile {
    required: Vec<Dependency>,
    replacements: HashMap<String, Replacement>,
}

impl ModuleFile {
    fn record(&mut self, directive: Directive, line: &str, number: usize) {
        match directive {
            Directive::Require => {
                let mut fields = line.split_whitespace();
                if let (Some(name), Some(version)) = (fields.next(), fields.next()) {
                    self.required.push(dependency(name, version, number));
                }
            }
            Directive::Replace => {
                if let Some((name, replacement)) = replacement(line) {
                    self.replacements.insert(name, replacement);
                }
            }
            Directive::Other => {}
        }
    }
}

pub fn module_file(text: &str) -> Vec<Dependency> {
    let mut module = ModuleFile::default();
    let mut block = Directive::Other;
    for (number, raw) in numbered_lines(text) {
        let line = raw.split(COMMENT).next().unwrap_or_default().trim();
        if block != Directive::Other {
            if line == BLOCK_CLOSE {
                block = Directive::Other;
            } else {
                module.record(block, line, number);
            }
            continue;
        }
        let (directive, rest) = directive_of(line);
        if rest == BLOCK_OPEN {
            block = directive;
        } else {
            module.record(directive, rest, number);
        }
    }
    let ModuleFile {
        required,
        replacements,
    } = module;
    required
        .into_iter()
        .filter_map(|dependency| replaced(dependency, &replacements))
        .collect()
}

pub fn checksums(text: &str) -> Vec<Dependency> {
    let mut seen = HashSet::new();
    numbered_lines(text)
        .filter_map(|(number, line)| {
            let mut fields = line.split_whitespace();
            let name = fields.next()?;
            let version = fields.next()?;
            if version.ends_with(MODULE_FILE_SUFFIX) {
                return None;
            }
            Some(dependency(name, version, number))
        })
        .filter(|dependency| seen.insert((dependency.name.clone(), dependency.version.clone())))
        .collect()
}

fn directive_of(line: &str) -> (Directive, &str) {
    if let Some(rest) = line.strip_prefix(REQUIRE) {
        return (Directive::Require, rest.trim());
    }
    if let Some(rest) = line.strip_prefix(REPLACE) {
        return (Directive::Replace, rest.trim());
    }
    (Directive::Other, line)
}

fn replacement(line: &str) -> Option<(String, Replacement)> {
    let (old, new) = line.split_once(ARROW)?;
    let mut old_fields = old.split_whitespace();
    let name = old_fields.next()?.to_owned();
    let version = old_fields.next().map(clean_version);
    let mut new_fields = new.split_whitespace();
    let target_name = new_fields.next()?;
    let target = if LOCAL_PATH_PREFIXES
        .iter()
        .any(|prefix| target_name.starts_with(prefix))
    {
        None
    } else {
        new_fields
            .next()
            .map(|target_version| (target_name.to_owned(), clean_version(target_version)))
    };
    Some((name, Replacement { version, target }))
}

fn replaced(dependency: Dependency, replacements: &HashMap<String, Replacement>) -> Option<Dependency> {
    let Some(replacement) = replacements.get(&dependency.name) else {
        return Some(dependency);
    };
    if replacement
        .version
        .as_ref()
        .is_some_and(|version| *version != dependency.version)
    {
        return Some(dependency);
    }
    let (name, version) = replacement.target.clone()?;
    Some(Dependency {
        name,
        version,
        ..dependency
    })
}

fn dependency(name: &str, version: &str, line: usize) -> Dependency {
    Dependency {
        ecosystem: Ecosystem::Go,
        name: name.to_owned(),
        version: clean_version(version),
        line,
    }
}

fn clean_version(version: &str) -> String {
    version.trim_start_matches(VERSION_PREFIX).to_owned()
}
