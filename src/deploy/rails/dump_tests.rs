use std::fs;
use std::path::PathBuf;

use super::rewrite::parse;
use super::{EXTENSION, evaluate, history};

const MIGRATE_VARIABLE: &str = "KERBO_RAILS_MIGRATE";
const OUTPUT_VARIABLE: &str = "KERBO_RAILS_SQL_OUTPUT";
const SQL_EXTENSION: &str = "sql";

#[test]
#[ignore = "lo corre scripts/parity/rails/dump.sh sobre un repo del corpus"]
fn dumps_the_rails_sql_of_every_migration() {
    let migrate = PathBuf::from(std::env::var(MIGRATE_VARIABLE).expect("falta la carpeta de migraciones"));
    let output = PathBuf::from(std::env::var(OUTPUT_VARIABLE).expect("falta la carpeta de salida"));
    let mut paths: Vec<PathBuf> = fs::read_dir(&migrate)
        .expect("se lee la carpeta")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == EXTENSION))
        .collect();
    paths.sort();
    for path in paths {
        let source = fs::read_to_string(&path).expect("se lee la migración");
        let tree = parse(&source).expect("el parser devuelve un árbol");
        let Some((script, _)) = evaluate(tree.root_node(), source.as_bytes(), history::schema_before(&path))
        else {
            continue;
        };
        let stem = path.file_stem().expect("tiene nombre");
        fs::write(output.join(stem).with_extension(SQL_EXTENSION), script.text()).expect("se escribe el SQL");
    }
}
