use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::SystemTime;

use tree_sitter::Parser;

use super::operations::Migration;
use super::parse::{Reader, migration};
use super::state::State;
use crate::language::Language;

const MIGRATIONS_DIRECTORY: &str = "migrations";
const PYTHON_EXTENSION: &str = "py";
const PACKAGE_STEM: &str = "__init__";
const IMPORT_MARKER: &str = "from django.db import";
const CLASS_MARKER: &str = "class Migration(";

pub trait Loader {
    fn migrations(&self, app: &str) -> Vec<(String, Arc<Migration>)>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub app: String,
    pub stem: String,
    pub apps_root: PathBuf,
}

pub fn location(path: &Path) -> Option<Location> {
    if path.extension()? != PYTHON_EXTENSION {
        return None;
    }
    let stem = path.file_stem()?.to_str()?;
    let directory = path.parent()?;
    if directory.file_name()? != MIGRATIONS_DIRECTORY || stem == PACKAGE_STEM {
        return None;
    }
    let app_directory = directory.parent()?;
    Some(Location {
        app: app_directory.file_name()?.to_str()?.to_owned(),
        stem: stem.to_owned(),
        apps_root: app_directory.parent()?.to_path_buf(),
    })
}

pub fn is_migration_source(source: &[u8]) -> bool {
    std::str::from_utf8(source).is_ok_and(|text| text.contains(IMPORT_MARKER) && text.contains(CLASS_MARKER))
}

pub fn state_before(loader: &impl Loader, location: &Location, current: &Migration) -> State {
    let mut state = State::default();
    for (stem, earlier) in loader.migrations(&location.app) {
        if stem < location.stem {
            replay(&mut state, &location.app, &earlier);
        }
    }
    load_related_apps(loader, &mut state, &location.app, current);
    state
}

fn replay(state: &mut State, app: &str, migration: &Migration) {
    for located in &migration.operations {
        state.apply(app, &located.operation);
    }
}

fn load_related_apps(loader: &impl Loader, state: &mut State, app: &str, current: &Migration) {
    let mut attempted = BTreeSet::from([app.to_owned()]);
    loop {
        let mut probe = state.clone();
        replay(&mut probe, app, current);
        let missing: BTreeSet<String> = probe
            .related_keys()
            .into_iter()
            .filter(|key| !probe.contains(key) && !attempted.contains(&key.app))
            .map(|key| key.app)
            .collect();
        if missing.is_empty() {
            return;
        }
        for other in missing {
            for (_, migration) in loader.migrations(&other) {
                replay(state, &other, &migration);
            }
            attempted.insert(other);
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileLoader {
    apps_root: PathBuf,
}

impl FileLoader {
    pub const fn new(apps_root: PathBuf) -> Self {
        Self { apps_root }
    }
}

impl Loader for FileLoader {
    fn migrations(&self, app: &str) -> Vec<(String, Arc<Migration>)> {
        let mut found: Vec<(String, Arc<Migration>)> = migration_files(&self.apps_root.join(app))
            .into_iter()
            .filter_map(|path| {
                let stem = path.file_stem()?.to_str()?.to_owned();
                Some((stem, cached(&path, app)?))
            })
            .collect();
        found.sort_by(|left, right| left.0.cmp(&right.0));
        found
    }
}

fn migration_files(app_directory: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(app_directory.join(MIGRATIONS_DIRECTORY)) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == PYTHON_EXTENSION)
                && path.file_stem().is_some_and(|stem| stem != PACKAGE_STEM)
        })
        .collect()
}

type Cache = Mutex<HashMap<PathBuf, (u64, SystemTime, Option<Arc<Migration>>)>>;

fn cache() -> &'static Cache {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    CACHE.get_or_init(Cache::default)
}

fn cached(path: &Path, app: &str) -> Option<Arc<Migration>> {
    let metadata = fs::metadata(path).ok()?;
    let (size, modified) = (metadata.len(), metadata.modified().ok()?);
    let known = cache()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(path)
        .filter(|(known_size, known_modified, _)| *known_size == size && *known_modified == modified)
        .map(|(_, _, parsed)| parsed.clone());
    if let Some(parsed) = known {
        return parsed;
    }
    let parsed = parse_file(path, app).map(Arc::new);
    cache()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(path.to_path_buf(), (size, modified, parsed.clone()));
    parsed
}

fn parse_file(path: &Path, app: &str) -> Option<Migration> {
    let source = fs::read(path).ok()?;
    if !is_migration_source(&source) {
        return None;
    }
    let mut parser = Parser::new();
    parser.set_language(&Language::Python.grammar()?).ok()?;
    let tree = parser.parse(&source, None)?;
    migration(tree.root_node(), Reader { source: &source, app })
}

pub fn fingerprint(path: &Path) -> Option<u64> {
    let location = location(path)?;
    let mut hasher = DefaultHasher::new();
    fs::read_dir(&location.apps_root)
        .ok()?
        .filter_map(Result::ok)
        .flat_map(|entry| migration_files(&entry.path()))
        .filter_map(|file| {
            let metadata = fs::metadata(&file).ok()?;
            Some((file, metadata.len(), metadata.modified().ok()?))
        })
        .collect::<BTreeSet<_>>()
        .hash(&mut hasher);
    Some(hasher.finish())
}
