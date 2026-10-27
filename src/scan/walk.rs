use std::path::{Component, Path};

use ignore::{DirEntry, Walk, WalkBuilder};

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
