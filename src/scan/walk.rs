use std::fs;
use std::path::{Component, Path, PathBuf};

use ignore::{DirEntry, Walk, WalkBuilder};

use crate::language::Language;

const ANALYZED_HIDDEN_DIRECTORIES: [&str; 2] = [".github", ".devcontainer"];
const GIT_DIRECTORY: &str = ".git";
const HIDDEN_PREFIX: char = '.';
const PATH_SEPARATOR: &str = "/";

pub fn walk(root: &Path) -> Walk {
    WalkBuilder::new(root)
        .hidden(false)
        .filter_entry(is_visited)
        .build()
}

// Un `.go` que es symlink a un archivo es parte del paquete de su carpeta, como en `go list`; en los
// demás lenguajes los symlinks no se recorren.
pub fn is_walked_symlink(path: &Path) -> bool {
    Language::from_path(path) == Some(Language::Go)
        && fs::metadata(path).is_ok_and(|metadata| metadata.is_file())
}

// La ruta con las carpetas resueltas pero sin seguir el archivo: un symlink queda en su carpeta.
pub fn canonical_location(path: &Path) -> PathBuf {
    let parent = path.parent().filter(|parent| !parent.as_os_str().is_empty());
    match (parent.map(fs::canonicalize), path.file_name()) {
        (Some(Ok(directory)), Some(name)) => directory.join(name),
        _ => path.canonicalize().unwrap_or_else(|_| path.to_path_buf()),
    }
}

fn is_visited(entry: &DirEntry) -> bool {
    entry.file_name() != GIT_DIRECTORY
}

// La ruta con `/` como separador en cualquier sistema.
pub fn portable(path: &Path) -> String {
    let parts: Vec<String> = path
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();
    parts.join(PATH_SEPARATOR)
}

pub fn is_analyzed(walk_root: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(walk_root) else {
        return true;
    };
    let mut components = relative.components().peekable();
    while let Some(component) = components.next() {
        let Component::Normal(name) = component else {
            continue;
        };
        let name = name.to_string_lossy();
        if !name.starts_with(HIDDEN_PREFIX) {
            continue;
        }
        let is_directory = components.peek().is_some();
        if !(is_directory && ANALYZED_HIDDEN_DIRECTORIES.contains(&name.as_ref())) {
            return false;
        }
    }
    true
}
