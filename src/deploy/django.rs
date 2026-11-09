mod alter;
mod columns;
mod condition;
mod deferred;
mod dependencies;
mod dispatch;
mod fields;
mod graph;
mod history;
mod md5;
mod meta;
mod model;
mod names;
mod operations;
mod parse;
mod rename;
mod run_python;
mod schema;
mod settings;
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
            settings: &settings::project(&location.apps_root),
        },
    ) else {
        return Vec::new();
    };
    let mut state = history::state_before(loader, location, &migration);
    dispatch::script(&migration, &location.app, &mut state)
}

#[cfg(test)]
fn rendered_sql(root: Node<'_>, source: &[u8], location: &Location) -> Option<String> {
    let migration = parse::migration(
        root,
        Reader {
            source,
            app: &location.app,
            settings: &settings::project(&location.apps_root),
        },
    )?;
    let loader = FileLoader::new(location.apps_root.clone());
    let mut state = history::state_before(&loader, location, &migration);
    Some(dispatch::sql(&migration, &location.app, &mut state))
}

#[cfg(test)]
mod tests;
