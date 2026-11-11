use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, PoisonError};

use super::discovery::{is_package, subdirectories};
use super::django_version::{Version, project_version};
use super::fields::class_of;
use super::installed_apps::installed_apps;
use super::model::{Class, ModelKey};
use super::project_fields::field_classes;
use super::settings_module::declared_modules;
use super::zero_downtime::{Backend, backend};

const SETTINGS_MODULE: &str = "settings";
const PYTHON_EXTENSION: &str = "py";
const USER_MODEL_SETTING: &str = "AUTH_USER_MODEL";
const AUTO_FIELD_SETTING: &str = "DEFAULT_AUTO_FIELD";
const DEFAULT_USER_APP: &str = "auth";
const DEFAULT_USER_MODEL: &str = "user";
const APP_SEPARATOR: char = '.';
const ASSIGNMENT: char = '=';
const ANNOTATION: char = ':';
const QUOTES: &[char] = &['"', '\''];
const SETTINGS_PREFIX: &str = "settings.";
const CASCADE_DROPPED_IN: Version = Version::new(6, 0);
const BIG_AUTO_FIELD_DEFAULT_IN: Version = Version::new(6, 0);
const RELATIVE_IMPORT: &str = "from .";
const IMPORT_KEYWORD: &str = " import ";
const WILDCARD: char = '*';
const PYTHON_TRUE: &str = "True";
const PYTHON_FALSE: &str = "False";
const CONFIG_MODULE: &str = "apps.py";
const MAXIMUM_SETTINGS_DEPTH: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub user_model: ModelKey,
    pub auto_field: Class,
    pub numbers: BTreeMap<String, String>,
    pub flags: BTreeMap<String, bool>,
    pub installed_apps: Option<BTreeSet<String>>,
    pub column_drops_cascade: bool,
    pub field_classes: BTreeMap<String, Class>,
    pub zero_downtime: Option<Backend>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            user_model: ModelKey::new(DEFAULT_USER_APP, DEFAULT_USER_MODEL),
            auto_field: Class::Auto,
            numbers: BTreeMap::new(),
            flags: BTreeMap::new(),
            installed_apps: None,
            column_drops_cascade: false,
            field_classes: BTreeMap::new(),
            zero_downtime: None,
        }
    }
}

impl Settings {
    fn of_a_new_project() -> Self {
        Self {
            auto_field: Class::BigAuto,
            ..Self::default()
        }
    }

    fn read(sources: &[String]) -> Self {
        let mut settings = Self::default();
        if let Some(model) = assigned(sources, USER_MODEL_SETTING).and_then(|value| {
            value
                .split_once(APP_SEPARATOR)
                .map(|(app, model)| ModelKey::new(app, model))
        }) {
            settings.user_model = model;
        }
        if let Some(class) = assigned(sources, AUTO_FIELD_SETTING)
            .and_then(|value| value.rsplit(APP_SEPARATOR).next().map(class_of))
            .filter(|class| *class != Class::Other)
        {
            settings.auto_field = class;
        }
        settings.numbers = numbers(sources);
        settings.flags = flags(sources);
        settings.installed_apps = installed_apps(sources);
        settings
    }

    pub fn replacement(&self, setting: &str, model: &ModelKey) -> Option<ModelKey> {
        (setting == USER_MODEL_SETTING && self.user_model != *model).then(|| self.user_model.clone())
    }

    pub fn includes_app(&self, app: &str) -> bool {
        self.installed_apps.as_ref().is_none_or(|apps| apps.contains(app))
    }

    pub fn number<'value>(&'value self, reference: &'value str) -> &'value str {
        reference
            .strip_prefix(SETTINGS_PREFIX)
            .and_then(|name| self.numbers.get(name))
            .map_or(reference, String::as_str)
    }

    pub fn flag(&self, name: &str) -> Option<bool> {
        self.flags.get(name).copied()
    }
}

fn is_constant_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|character| character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_')
}

fn is_integer(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|character| character.is_ascii_digit())
}

fn is_identifier(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|character| character.is_alphanumeric() || character == '_')
}

fn split_assignment(line: &str) -> Option<(Vec<&str>, &str)> {
    let mut targets = Vec::new();
    let mut rest = line.trim_start();
    while let Some((target, after)) = rest.split_once(ASSIGNMENT) {
        if after.starts_with(ASSIGNMENT) {
            break;
        }
        let name = target
            .split_once(ANNOTATION)
            .map_or(target, |(name, _)| name)
            .trim();
        if !is_identifier(name) {
            break;
        }
        targets.push(name);
        rest = after.trim_start();
    }
    (!targets.is_empty()).then(|| (targets, rest.trim_end()))
}

fn constant_assignments(sources: &[String]) -> impl Iterator<Item = (&str, &str)> {
    sources
        .iter()
        .flat_map(|source| source.lines())
        .filter(|line| !line.starts_with(char::is_whitespace))
        .filter_map(split_assignment)
        .flat_map(|(targets, value)| targets.into_iter().map(move |name| (name, value)))
        .filter(|(name, _)| is_constant_name(name))
}

fn numbers(sources: &[String]) -> BTreeMap<String, String> {
    constant_assignments(sources)
        .filter(|(_, value)| is_integer(value))
        .map(|(name, value)| (name.to_owned(), value.to_owned()))
        .collect()
}

fn flags(sources: &[String]) -> BTreeMap<String, bool> {
    constant_assignments(sources)
        .filter_map(|(name, value)| match value {
            PYTHON_TRUE => Some((name.to_owned(), true)),
            PYTHON_FALSE => Some((name.to_owned(), false)),
            _ => None,
        })
        .collect()
}

fn assigned_text<'source>(line: &'source str, name: &str) -> Option<&'source str> {
    let (targets, value) = split_assignment(line)?;
    if !targets.contains(&name) {
        return None;
    }
    value.strip_prefix(QUOTES)?.strip_suffix(QUOTES)
}

fn assigned(sources: &[String], name: &str) -> Option<String> {
    let lines = || sources.iter().flat_map(|source| source.lines());
    let is_top_level = |line: &&str| !line.starts_with(char::is_whitespace);
    lines()
        .filter(is_top_level)
        .find_map(|line| assigned_text(line, name))
        .or_else(|| {
            lines()
                .filter(|line| !is_top_level(line))
                .filter_map(|line| assigned_text(line, name))
                .next_back()
        })
        .map(str::to_owned)
}

fn candidates(apps_root: &Path) -> Vec<PathBuf> {
    let mut packages = vec![apps_root.to_path_buf()];
    if let Ok(entries) = fs::read_dir(apps_root) {
        packages.extend(
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.is_dir()),
        );
    }
    let mut files = Vec::new();
    for package in packages {
        files.push(package.join(SETTINGS_MODULE).with_extension(PYTHON_EXTENSION));
        let settings_package = package.join(SETTINGS_MODULE);
        if settings_package.join(CONFIG_MODULE).is_file() {
            continue;
        }
        if let Ok(entries) = fs::read_dir(settings_package) {
            files.extend(
                entries
                    .filter_map(Result::ok)
                    .map(|entry| entry.path())
                    .filter(|path| {
                        path.extension()
                            .is_some_and(|extension| extension == PYTHON_EXTENSION)
                    }),
            );
        }
    }
    files.retain(|file| file.is_file());
    files.sort();
    files
}

fn settings_files(source_root: &Path) -> Vec<PathBuf> {
    let mut level = vec![source_root.to_path_buf()];
    for _ in 0..=MAXIMUM_SETTINGS_DEPTH {
        let mut next = Vec::new();
        for directory in &level {
            let files = candidates(directory);
            if !files.is_empty() {
                return files;
            }
            next.extend(
                subdirectories(directory)
                    .into_iter()
                    .filter(|child| is_package(child)),
            );
        }
        level = next;
    }
    Vec::new()
}

fn wildcard_imports(source: &str, directory: &Path) -> Vec<PathBuf> {
    source
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix(RELATIVE_IMPORT))
        .filter_map(|rest| rest.split_once(IMPORT_KEYWORD))
        .filter(|(_, imported)| imported.trim_start().starts_with(WILDCARD))
        .map(|(module, _)| {
            module
                .split(APP_SEPARATOR)
                .fold(directory.to_path_buf(), |path, segment| path.join(segment))
                .with_extension(PYTHON_EXTENSION)
        })
        .collect()
}

fn sources_from(files: Vec<PathBuf>) -> Vec<String> {
    let mut pending = files;
    pending.reverse();
    let mut visited = BTreeSet::new();
    let mut sources = Vec::new();
    while let Some(path) = pending.pop() {
        if !visited.insert(path.clone()) {
            continue;
        }
        let Ok(source) = fs::read_to_string(&path) else {
            continue;
        };
        if let Some(directory) = path.parent() {
            let mut imported = wildcard_imports(&source, directory);
            imported.reverse();
            pending.extend(imported);
        }
        sources.push(source);
    }
    sources
}

fn discovered(apps_root: &Path) -> Settings {
    let mut sources = sources_from(settings_files(apps_root));
    if installed_apps(&sources).is_none()
        && let Some(declared) = declared_modules(apps_root)
            .into_iter()
            .map(|file| sources_from(vec![file]))
            .find(|candidate| installed_apps(candidate).is_some())
    {
        sources = declared;
    }
    let version = project_version(apps_root);
    let mut settings = if sources.is_empty() {
        Settings::of_a_new_project()
    } else {
        Settings::read(&sources)
    };
    let big_by_default = version.is_some_and(|known| known >= BIG_AUTO_FIELD_DEFAULT_IN);
    if big_by_default && assigned(&sources, AUTO_FIELD_SETTING).is_none() {
        settings.auto_field = Class::BigAuto;
    }
    settings.field_classes = field_classes(apps_root);
    settings.zero_downtime = backend(&sources, apps_root);
    settings.column_drops_cascade = version.is_some_and(|known| known < CASCADE_DROPPED_IN);
    settings
}

pub fn project(apps_root: &Path) -> Settings {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Settings>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Mutex::default);
    if let Some(known) = cache
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(apps_root)
    {
        return known.clone();
    }
    let settings = discovered(apps_root);
    cache
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(apps_root.to_path_buf(), settings.clone());
    settings
}

#[cfg(test)]
mod tests;
