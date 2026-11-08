mod alter;
mod columns;
mod deferred;
mod dispatch;
mod fields;
mod history;
mod md5;
mod meta;
mod model;
mod names;
mod operations;
mod parse;
mod schema;
mod state;
mod values;

use std::path::Path;

use tree_sitter::Node;

use super::Violation;
use history::{FileLoader, Loader, Location};
use parse::Reader;

pub use history::fingerprint;

pub fn violations(root: Node<'_>, source: &[u8], path: Option<&Path>) -> Vec<Violation> {
    let Some(location) = path.and_then(history::location) else {
        return Vec::new();
    };
    if !history::is_migration_source(source) {
        return Vec::new();
    }
    let loader = FileLoader::new(location.apps_root.clone());
    render(root, source, &location, &loader)
}

fn render(root: Node<'_>, source: &[u8], location: &Location, loader: &impl Loader) -> Vec<Violation> {
    let Some(migration) = parse::migration(
        root,
        Reader {
            source,
            app: &location.app,
        },
    ) else {
        return Vec::new();
    };
    let mut state = history::state_before(loader, location, &migration);
    dispatch::script(&migration, &location.app, &mut state)
}

#[cfg(test)]
mod tests;
