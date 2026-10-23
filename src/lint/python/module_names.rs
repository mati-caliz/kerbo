use std::path::Path;

use tree_sitter::Point;

use super::Report;
use super::ast::is_keyword;
use super::imports::package_root;
use crate::rules::PythonRule;

const MODULE_FILES: [&str; 4] = ["__init__.py", "__init__.pyi", "__main__.py", "__main__.pyi"];
const MIGRATION_DIRECTORIES: [&str; 2] = ["versions", "migrations"];
const IGNORED_NAMES: [&str; 12] = [
    "setUp",
    "tearDown",
    "setUpClass",
    "tearDownClass",
    "setUpModule",
    "tearDownModule",
    "asyncSetUp",
    "asyncTearDown",
    "setUpTestData",
    "failureException",
    "longMessage",
    "maxDiff",
];
const UNDERSCORE: char = '_';

// N999: un módulo de un paquete cuyo nombre no es un identificador en minúsculas. Un `__init__.py`
// o `__main__.py` toma el nombre del paquete raíz, como ruff, y en `migrations` o `versions` el
// nombre puede empezar con un dígito (`0001_initial.py`).
pub fn invalid_module_name(path: Option<&Path>) -> Option<Report> {
    let path = path?;
    let package = path.parent().and_then(package_root)?;
    let file_name = path.file_name()?.to_str()?;
    let module_name = if MODULE_FILES.contains(&file_name) {
        package.file_name()?.to_str()?
    } else {
        path.file_stem()?.to_str()?
    };
    let is_valid = if is_migration_file(path) {
        is_migration_name(module_name)
    } else {
        is_module_name(module_name)
    };
    (!is_valid && !IGNORED_NAMES.contains(&module_name))
        .then(|| Report::at_point(PythonRule::N999, Point::new(0, 0), module_name))
}

fn is_migration_file(path: &Path) -> bool {
    path.parent()
        .and_then(Path::file_name)
        .and_then(|directory| directory.to_str())
        .is_some_and(|directory| MIGRATION_DIRECTORIES.contains(&directory))
}

fn is_module_name(name: &str) -> bool {
    name.chars()
        .next()
        .is_some_and(|first| first.is_ascii_lowercase() || first == UNDERSCORE)
        && is_migration_name(name)
}

fn is_migration_name(name: &str) -> bool {
    name.chars().all(|character| {
        character.is_ascii_lowercase() || character.is_ascii_digit() || character == UNDERSCORE
    }) && !is_keyword(name)
}
