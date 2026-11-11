mod alter;
mod columns;
mod computed_names;
mod condition;
mod constants;
mod deferred;
mod dependencies;
mod discovery;
mod dispatch;
mod django_version;
mod enum_members;
mod expression;
mod factories;
mod field_defaults;
mod fields;
mod graph;
mod history;
mod installed_apps;
mod md5;
mod meta;
mod migration_base;
mod migration_references;
mod model;
mod names;
mod operations;
mod parse;
mod project_fields;
mod rename;
mod run_python;
mod schema;
mod settings;
mod settings_module;
mod state;
mod table_constraints;
mod truth;
mod values;
mod zero_downtime;

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
    let loader = FileLoader::new(location.source_root.clone());
    render(root, source, &location, &loader)
}

fn render(root: Node<'_>, source: &[u8], location: &Location, loader: &impl Loader) -> Vec<Violation> {
    let Some(migration) = parse::migration(
        root,
        Reader {
            source,
            app: &location.app,
            settings: &settings::project(&location.source_root),
            apps_root: Some(&location.apps_root),
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
            settings: &settings::project(&location.source_root),
            apps_root: Some(&location.apps_root),
        },
    )?;
    let loader = FileLoader::new(location.source_root.clone());
    let mut state = history::state_before(&loader, location, &migration);
    Some(dispatch::sql(&migration, &location.app, &mut state))
}

#[cfg(test)]
mod tests;
