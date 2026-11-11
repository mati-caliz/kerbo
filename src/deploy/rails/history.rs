use std::collections::{BTreeSet, HashMap};
use std::ffi::OsString;
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use tree_sitter::Parser;

use super::schema::Schema;
use super::{EXTENSION, evaluate, is_migration};
use crate::language::Language;

type Snapshots = Arc<HashMap<OsString, Schema>>;
type Cache = Mutex<HashMap<PathBuf, (u64, Snapshots)>>;

fn cache() -> &'static Cache {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    CACHE.get_or_init(Cache::default)
}

fn siblings(directory: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(directory)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == EXTENSION))
        .collect();
    files.sort();
    files
}

fn directory_fingerprint(files: &[PathBuf]) -> u64 {
    let mut hasher = DefaultHasher::new();
    files
        .iter()
        .filter_map(|file| {
            let metadata = fs::metadata(file).ok()?;
            Some((file.clone(), metadata.len(), metadata.modified().ok()?))
        })
        .collect::<BTreeSet<_>>()
        .hash(&mut hasher);
    hasher.finish()
}

pub fn fingerprint(path: &Path) -> Option<u64> {
    if !is_migration(path) {
        return None;
    }
    Some(directory_fingerprint(&siblings(path.parent()?)))
}

fn replay(files: &[PathBuf]) -> HashMap<OsString, Schema> {
    let mut parser = Parser::new();
    let grammar = Language::Rails.grammar();
    let ready = grammar.is_some_and(|grammar| parser.set_language(&grammar).is_ok());
    let mut schema = Schema::complete();
    let mut snapshots = HashMap::new();
    for file in files {
        let Some(name) = file.file_name() else {
            continue;
        };
        snapshots.insert(name.to_owned(), schema.clone());
        let Some(source) = ready.then(|| fs::read(file).ok()).flatten() else {
            continue;
        };
        if let Some(tree) = parser.parse(&source, None)
            && let Some((_, after)) = evaluate(tree.root_node(), &source, schema.clone())
        {
            schema = after;
        }
    }
    snapshots
}

pub fn schema_before(path: &Path) -> Schema {
    let (Some(directory), Some(name)) = (path.parent(), path.file_name()) else {
        return Schema::default();
    };
    if !is_migration(path) {
        return Schema::default();
    }
    let files = siblings(directory);
    let current = directory_fingerprint(&files);
    let known = cache()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(directory)
        .filter(|(known, _)| *known == current)
        .map(|(_, snapshots)| Arc::clone(snapshots));
    let snapshots = known.unwrap_or_else(|| {
        let snapshots = Arc::new(replay(&files));
        cache()
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(directory.to_path_buf(), (current, Arc::clone(&snapshots)));
        snapshots
    });
    snapshots.get(name).cloned().unwrap_or_default()
}
