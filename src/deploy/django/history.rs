use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::SystemTime;

use tree_sitter::Parser;

use super::graph::{Graph, Nodes, Parents, ancestors};
use super::operations::{Migration, MigrationKey};
use super::parse::{Reader, migration};
use super::settings::{self, Settings};
use super::state::State;
use crate::language::Language;

const MIGRATIONS_DIRECTORY: &str = "migrations";
const PYTHON_EXTENSION: &str = "py";
const PACKAGE_STEM: &str = "__init__";
const APPS_ROOT_DEPTH: usize = 3;
const IMPORT_MARKER: &str = "from django.db import";
const CLASS_MARKER: &str = "class Migration(";

pub trait Loader {
    fn apps(&self) -> Vec<String>;
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
    let settings = settings::project(&location.apps_root);
    let mut state = State::new(settings.auto_field, settings.column_drops_cascade);
    let current_key = MigrationKey::new(&location.app, &location.stem);
    let mut nodes: Nodes = installed(loader, &settings, &location.app)
        .iter()
        .flat_map(|app| {
            loader
                .migrations(app)
                .into_iter()
                .map(move |(stem, migration)| (MigrationKey::new(app, &stem), migration))
        })
        .collect();
    nodes.insert(current_key.clone(), Arc::new(current.clone()));
    let project = project(&location.apps_root, nodes);
    for key in ancestors(&project.parents, &current_key) {
        if let Some(ancestor) = project.nodes.get(key) {
            for located in &ancestor.operations {
                state.apply(&key.app, &located.operation);
            }
        }
    }
    state
}

fn installed(loader: &impl Loader, settings: &Settings, current_app: &str) -> Vec<String> {
    let apps = loader.apps();
    let mut included: Vec<String> = apps
        .iter()
        .filter(|app| settings.includes_app(app))
        .cloned()
        .collect();
    if included.is_empty() {
        included = apps;
    }
    if !included.iter().any(|app| app == current_app) {
        included.push(current_app.to_owned());
    }
    included
}

struct Project {
    nodes: Nodes,
    parents: Parents,
}

fn same_nodes(known: &Nodes, nodes: &Nodes) -> bool {
    known.len() == nodes.len()
        && known
            .iter()
            .zip(nodes)
            .all(|((known_key, known_migration), (key, migration))| {
                known_key == key && (Arc::ptr_eq(known_migration, migration) || known_migration == migration)
            })
}

fn project(apps_root: &Path, nodes: Nodes) -> Arc<Project> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Arc<Project>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Mutex::default);
    let known = cache
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(apps_root)
        .filter(|known| same_nodes(&known.nodes, &nodes))
        .cloned();
    if let Some(known) = known {
        return known;
    }
    let parents = Graph::new(&nodes).into_parents();
    let built = Arc::new(Project { nodes, parents });
    cache
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(apps_root.to_path_buf(), Arc::clone(&built));
    built
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
    fn apps(&self) -> Vec<String> {
        let Ok(entries) = fs::read_dir(&self.apps_root) else {
            return Vec::new();
        };
        entries
            .filter_map(Result::ok)
            .filter(|entry| entry.path().join(MIGRATIONS_DIRECTORY).is_dir())
            .filter_map(|entry| entry.file_name().to_str().map(str::to_owned))
            .collect()
    }

    fn migrations(&self, app: &str) -> Vec<(String, Arc<Migration>)> {
        let settings = settings::project(&self.apps_root);
        let mut found: Vec<(String, Arc<Migration>)> = migration_files(&self.apps_root.join(app))
            .into_iter()
            .filter_map(|path| {
                let stem = path.file_stem()?.to_str()?.to_owned();
                Some((stem, cached(&path, app, &settings)?))
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

fn cached(path: &Path, app: &str, settings: &Settings) -> Option<Arc<Migration>> {
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
    let parsed = parse_file(path, app, settings).map(Arc::new);
    cache()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(path.to_path_buf(), (size, modified, parsed.clone()));
    parsed
}

fn parse_file(path: &Path, app: &str, settings: &Settings) -> Option<Migration> {
    let source = fs::read(path).ok()?;
    if !is_migration_source(&source) {
        return None;
    }
    let mut parser = Parser::new();
    parser.set_language(&Language::Python.grammar()?).ok()?;
    let tree = parser.parse(&source, None)?;
    migration(
        tree.root_node(),
        Reader {
            source: &source,
            app,
            settings,
            apps_root: path.ancestors().nth(APPS_ROOT_DEPTH),
        },
    )
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
