mod model;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use super::{Dependency, numbered_lines};
use crate::osv::Ecosystem;
use model::{Declared, PomModel};

const POM_FILE: &str = "pom.xml";
const DEFAULT_PARENT_PATH: &str = "../pom.xml";
const MAX_ANCESTORS: usize = 8;
const MAX_INTERPOLATIONS: usize = 8;
const PROJECT_VERSION: &str = "project.version";
const PROJECT_GROUP: &str = "project.groupId";
const PARENT_VERSION: &str = "project.parent.version";
const PLACEHOLDER_START: &str = "${";
const PLACEHOLDER_END: char = '}';
const COORDINATE_SEPARATOR: char = ':';
const GRADLE_COMMENT: char = '#';
const GRADLE_CONFIGURATIONS: char = '=';
const FIRST_LINE: usize = 1;

pub fn pom(path: &Path, text: &str) -> Vec<Dependency> {
    let chain = ancestry(path, PomModel::parse(text));
    let properties = merged_properties(&chain);
    let mut managed: HashMap<String, String> = HashMap::new();
    for declared in chain.iter().flat_map(|model| &model.managed) {
        if let Some(version) = &declared.version {
            managed
                .entry(coordinates(declared))
                .or_insert_with(|| version.clone());
        }
    }
    let inherited_line = chain
        .first()
        .and_then(|model| model.parent.as_ref())
        .map_or(FIRST_LINE, |parent| parent.line);
    let mut seen = HashSet::new();
    chain
        .iter()
        .enumerate()
        .flat_map(|(depth, model)| {
            let line_of = move |declared: &Declared| if depth == 0 { declared.line } else { inherited_line };
            model
                .dependencies
                .iter()
                .map(move |declared| (declared, line_of(declared)))
        })
        .filter_map(|(declared, line)| {
            let (name, version) = resolved(declared, &managed, &properties)?;
            Some(Dependency {
                ecosystem: Ecosystem::Maven,
                name,
                version,
                line,
            })
        })
        .filter(|dependency| seen.insert(dependency.name.clone()))
        .collect()
}

fn resolved(
    declared: &Declared,
    managed: &HashMap<String, String>,
    properties: &HashMap<String, String>,
) -> Option<(String, String)> {
    let name = interpolate(&coordinates(declared), properties)?;
    let raw = declared
        .version
        .clone()
        .or_else(|| managed.get(&coordinates(declared)).cloned())?;
    Some((name, interpolate(&raw, properties)?))
}

fn ancestry(path: &Path, child: PomModel) -> Vec<PomModel> {
    let mut chain = vec![child];
    let mut current = path.to_path_buf();
    while chain.len() <= MAX_ANCESTORS {
        let Some(next) = chain.last().and_then(|model| parent_path(&current, model)) else {
            break;
        };
        let Ok(text) = fs::read_to_string(&next) else {
            break;
        };
        let loaded = PomModel::parse(&text);
        if !chain
            .last()
            .is_some_and(|model| is_declared_parent(model, &loaded))
        {
            break;
        }
        chain.push(loaded);
        current = next;
    }
    chain
}

fn is_declared_parent(child: &PomModel, loaded: &PomModel) -> bool {
    let Some(declared) = child.parent.as_ref() else {
        return false;
    };
    let inherited = loaded.parent.as_ref();
    let group = loaded
        .group
        .as_ref()
        .or_else(|| inherited.map(|parent| &parent.group));
    let version = loaded
        .version
        .as_ref()
        .or_else(|| inherited.map(|parent| &parent.version));
    loaded.artifact.as_ref() == Some(&declared.artifact)
        && group == Some(&declared.group)
        && version == Some(&declared.version)
}

fn parent_path(current: &Path, model: &PomModel) -> Option<PathBuf> {
    let parent = model.parent.as_ref()?;
    let relative = parent.relative_path.as_deref().unwrap_or(DEFAULT_PARENT_PATH);
    let candidate = current.parent()?.join(relative);
    Some(if candidate.is_dir() {
        candidate.join(POM_FILE)
    } else {
        candidate
    })
}

fn merged_properties(chain: &[PomModel]) -> HashMap<String, String> {
    let mut properties = HashMap::new();
    for model in chain.iter().rev() {
        properties.extend(model.properties.clone());
    }
    if let Some(child) = chain.first() {
        let parent = child.parent.as_ref();
        let version = child
            .version
            .clone()
            .or_else(|| parent.map(|parent| parent.version.clone()));
        let group = child
            .group
            .clone()
            .or_else(|| parent.map(|parent| parent.group.clone()));
        properties.extend(version.map(|version| (PROJECT_VERSION.to_owned(), version)));
        properties.extend(group.map(|group| (PROJECT_GROUP.to_owned(), group)));
        properties.extend(parent.map(|parent| (PARENT_VERSION.to_owned(), parent.version.clone())));
    }
    properties
}

fn coordinates(declared: &Declared) -> String {
    format!("{}{COORDINATE_SEPARATOR}{}", declared.group, declared.artifact)
}

fn interpolate(value: &str, properties: &HashMap<String, String>) -> Option<String> {
    let mut current = value.to_owned();
    for _ in 0..MAX_INTERPOLATIONS {
        let Some(start) = current.find(PLACEHOLDER_START) else {
            return Some(current);
        };
        let rest = current.get(start.saturating_add(PLACEHOLDER_START.len())..)?;
        let end = rest.find(PLACEHOLDER_END)?;
        let key = rest.get(..end)?;
        let replacement = properties.get(key)?;
        current = format!(
            "{}{replacement}{}",
            current.get(..start)?,
            rest.get(end.saturating_add(1)..)?
        );
    }
    None
}

pub fn gradle_lockfile(text: &str) -> Vec<Dependency> {
    numbered_lines(text)
        .filter_map(|(number, line)| {
            if line.starts_with(GRADLE_COMMENT) {
                return None;
            }
            let coordinates = line.split(GRADLE_CONFIGURATIONS).next()?;
            let mut parts = coordinates.split(COORDINATE_SEPARATOR);
            let (group, artifact, version) = (parts.next()?, parts.next()?, parts.next()?);
            Some(Dependency {
                ecosystem: Ecosystem::Maven,
                name: format!("{group}{COORDINATE_SEPARATOR}{artifact}"),
                version: version.trim().to_owned(),
                line: number,
            })
        })
        .collect()
}
