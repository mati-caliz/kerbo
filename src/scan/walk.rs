use std::path::Path;

use ignore::{DirEntry, Walk, WalkBuilder};

const VISITED_HIDDEN_DIRECTORIES: [&str; 2] = [".github", ".devcontainer"];
const HIDDEN_PREFIX: char = '.';
const WALK_ROOT_DEPTH: usize = 0;

pub fn walk(root: &Path) -> Walk {
    WalkBuilder::new(root)
        .hidden(false)
        .filter_entry(is_visited)
        .build()
}

fn is_visited(entry: &DirEntry) -> bool {
    if entry.depth() == WALK_ROOT_DEPTH {
        return true;
    }
    let name = entry.file_name().to_string_lossy();
    if !name.starts_with(HIDDEN_PREFIX) {
        return true;
    }
    let is_directory = entry.file_type().is_some_and(|file_type| file_type.is_dir());
    is_directory && VISITED_HIDDEN_DIRECTORIES.contains(&name.as_ref())
}
