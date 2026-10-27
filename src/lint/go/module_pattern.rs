use std::ffi::OsStr;
use std::path::Path;

const MODULE_FILE: &str = "go.mod";
const SKIPPED_DIRECTORIES: &[&str] = &["testdata", "vendor"];
const SKIPPED_PREFIXES: &[char] = &['_', '.'];

// Si `./...` desde la raíz del módulo (el `go.mod` más cercano) llega al archivo: el patrón saltea
// `testdata`, `vendor` y las carpetas que empiezan con `_` o `.`. Sin `go.mod` no se descarta nada.
pub fn listed_by_module_pattern(path: &Path) -> bool {
    let Some(directory) = path.parent() else {
        return true;
    };
    let mut skipped = false;
    for ancestor in directory.ancestors() {
        if ancestor.join(MODULE_FILE).is_file() {
            return !skipped;
        }
        skipped |= ancestor
            .file_name()
            .and_then(OsStr::to_str)
            .is_some_and(is_skipped_directory);
    }
    true
}

fn is_skipped_directory(name: &str) -> bool {
    SKIPPED_DIRECTORIES.contains(&name) || name.starts_with(SKIPPED_PREFIXES)
}

#[cfg(test)]
mod tests;
