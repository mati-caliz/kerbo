use std::fs;
use std::path::{Path, PathBuf};

use super::parsed;
use crate::deploy::django::history::location;
use crate::deploy::django::rendered_sql;

const APPS_ROOT_VARIABLE: &str = "KERBO_DJANGO_APPS_ROOT";
const OUTPUT_VARIABLE: &str = "KERBO_DJANGO_SQL_OUTPUT";
const MIGRATIONS_DIRECTORY: &str = "migrations";
const SQL_EXTENSION: &str = "sql";

fn migration_paths(apps_root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut pending = vec![apps_root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        for path in entries.filter_map(Result::ok).map(|entry| entry.path()) {
            if !path.is_dir() {
                continue;
            }
            if path.file_name().is_some_and(|name| name == MIGRATIONS_DIRECTORY) {
                paths.extend(
                    fs::read_dir(&path)
                        .into_iter()
                        .flatten()
                        .filter_map(Result::ok)
                        .map(|entry| entry.path()),
                );
            } else {
                pending.push(path);
            }
        }
    }
    paths.sort();
    paths
}

#[test]
#[ignore = "lo corre scripts/parity/django/dump.sh sobre un repo del corpus"]
fn dumps_the_sql_of_every_migration() {
    let apps_root = PathBuf::from(std::env::var(APPS_ROOT_VARIABLE).expect("falta la carpeta de apps"));
    let output = PathBuf::from(std::env::var(OUTPUT_VARIABLE).expect("falta la carpeta de salida"));
    for path in migration_paths(&apps_root) {
        let Some(found) = location(&path) else {
            continue;
        };
        let source = fs::read(&path).expect("se lee la migración");
        let tree = parsed(&String::from_utf8_lossy(&source));
        let Some(sql) = rendered_sql(tree.root_node(), &source, &found) else {
            continue;
        };
        let target = output
            .join(&found.app)
            .join(&found.stem)
            .with_extension(SQL_EXTENSION);
        fs::create_dir_all(target.parent().expect("tiene carpeta")).expect("se crea la carpeta");
        fs::write(target, sql).expect("se escribe el SQL");
    }
}
