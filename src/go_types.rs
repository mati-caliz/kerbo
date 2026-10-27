mod helper;

use std::collections::HashMap;
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::Deserialize;

use crate::cache::location::default_directory;
use crate::lint::go::platform::GoPlatform;

const TYPES_DIRECTORY: &str = "go-types";
const FACTS_DIRECTORY: &str = "facts";
const FACTS_EXTENSION: &str = "jsonl";
const GO_EXTENSION: &str = "go";
const MODULE_FILES: &[&str] = &["go.mod", "go.sum", "go.work", "go.work.sum"];
const SKIPPED_DIRECTORIES: &[&str] = &[".git", "node_modules", "target"];

// Una llamada que devuelve un `error`, con lo que errcheck mira para excluirla: los nombres de la
// función (también por las interfaces embebidas), el tipo del primer argumento y cómo se escribió.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct CallFact {
    pub line: usize,
    pub column: usize,
    #[serde(default)]
    pub names: Option<Vec<String>>,
    #[serde(default)]
    pub argument: String,
    #[serde(default)]
    pub errors_as_type: bool,
    #[serde(default)]
    pub function: String,
    #[serde(default)]
    pub selector: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct PositionFact {
    pub line: usize,
    pub column: usize,
}

// Un `for range` sobre una función que entrega un `error` que el `for` no recibe.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct RangeFact {
    pub line: usize,
    pub column: usize,
    #[serde(default)]
    pub call: Option<CallFact>,
}

// Una escritura que QF1012 pasa a fmt.Fprint*, con la función que la reemplaza.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct FprintFact {
    pub line: usize,
    pub column: usize,
    pub replacement: String,
}

// Un aviso que el helper decide entero, con la regla y lo que nombra.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct ReportFact {
    pub rule: String,
    pub line: usize,
    pub column: usize,
    #[serde(default)]
    pub subject: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct FileFacts {
    pub file: PathBuf,
    #[serde(default)]
    pub calls: Option<Vec<CallFact>>,
    #[serde(default)]
    pub receives: Option<Vec<PositionFact>>,
    #[serde(default)]
    pub ranges: Option<Vec<RangeFact>>,
    #[serde(default)]
    pub embedded: Option<Vec<PositionFact>>,
    #[serde(default)]
    pub fprints: Option<Vec<FprintFact>>,
    #[serde(default)]
    pub reports: Option<Vec<ReportFact>>,
}

// Los hechos de tipos de un módulo, por archivo (con la ruta canónica).
#[derive(Debug, Default)]
pub struct ModuleFacts {
    files: HashMap<PathBuf, FileFacts>,
}

impl ModuleFacts {
    pub fn parse(lines: &str) -> Self {
        let mut files: HashMap<PathBuf, FileFacts> = HashMap::new();
        for facts in lines
            .lines()
            .filter_map(|line| serde_json::from_str::<FileFacts>(line).ok())
        {
            let key = canonical(&facts.file);
            let merged = files.entry(key).or_default();
            merge(merged, facts);
        }
        Self { files }
    }

    pub fn file(&self, path: &Path) -> Option<&FileFacts> {
        self.files.get(&canonical(path))
    }
}

fn merge(into: &mut FileFacts, facts: FileFacts) {
    into.file = facts.file;
    extend(&mut into.calls, facts.calls);
    extend(&mut into.receives, facts.receives);
    extend(&mut into.ranges, facts.ranges);
    extend(&mut into.embedded, facts.embedded);
    extend(&mut into.fprints, facts.fprints);
    extend(&mut into.reports, facts.reports);
}

fn extend<Item>(into: &mut Option<Vec<Item>>, more: Option<Vec<Item>>) {
    if let Some(more) = more {
        into.get_or_insert_with(Vec::new).extend(more);
    }
}

fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

// Los hechos del módulo: de la caché si ningún `.go` ni el `go.mod` cambiaron, si no del helper.
pub fn module_facts(module_root: &Path, platform: &GoPlatform) -> Result<ModuleFacts, String> {
    let directory = default_directory()
        .map(|cache| cache.join(TYPES_DIRECTORY))
        .ok_or_else(|| "no hay carpeta de caché".to_owned())?;
    let saved = directory
        .join(FACTS_DIRECTORY)
        .join(format!("{:016x}", fingerprint(module_root, platform)))
        .with_extension(FACTS_EXTENSION);
    if let Ok(contents) = fs::read_to_string(&saved) {
        return Ok(ModuleFacts::parse(&contents));
    }
    let output = helper::facts(&directory, module_root, platform)?;
    if output.complete
        && saved
            .parent()
            .is_some_and(|parent| fs::create_dir_all(parent).is_ok())
    {
        drop(fs::write(&saved, &output.lines));
    }
    Ok(ModuleFacts::parse(&output.lines))
}

// La huella del módulo: el helper, la plataforma y cada `.go` y archivo de módulo con su tamaño y
// fecha de modificación.
fn fingerprint(module_root: &Path, platform: &GoPlatform) -> u64 {
    let mut hasher = DefaultHasher::new();
    helper::SOURCE.hash(&mut hasher);
    platform.hash(&mut hasher);
    let mut files = module_files(module_root);
    files.sort();
    for file in files {
        file.hash(&mut hasher);
        if let Ok(metadata) = fs::metadata(&file) {
            metadata.len().hash(&mut hasher);
            metadata
                .modified()
                .ok()
                .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
                .hash(&mut hasher);
        }
    }
    hasher.finish()
}

fn module_files(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let is_directory = path.is_dir();
            if is_directory && !SKIPPED_DIRECTORIES.contains(&name.as_str()) {
                pending.push(path);
            } else if !is_directory && is_module_file(&path, &name) {
                found.push(path);
            }
        }
    }
    found
}

fn is_module_file(path: &Path, name: &str) -> bool {
    path.extension()
        .is_some_and(|extension| extension == GO_EXTENSION)
        || MODULE_FILES.contains(&name)
}

#[cfg(test)]
mod tests;
