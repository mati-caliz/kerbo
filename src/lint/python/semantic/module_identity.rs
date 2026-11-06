use std::path::Path;

use crate::lint::python::imports::package_root;

const IMPORT_SEPARATOR: char = '.';

// El módulo del archivo, como `Module` de ruff: su ruta desde la raíz del paquete o, en un script
// suelto, sólo su nombre. Con eso, una función de `gzip.py` es `gzip.open`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModuleIdentity {
    path: Option<Vec<String>>,
    name: Option<String>,
}

impl ModuleIdentity {
    pub fn of(file: Option<&Path>) -> Self {
        let Some(file) = file else {
            return Self::default();
        };
        let path = file
            .parent()
            .and_then(package_root)
            .and_then(|package| to_module_path(&package, file));
        let name = file.file_stem().map(|stem| stem.to_string_lossy().into_owned());
        Self { path, name }
    }

    // `resolve_qualified_name` para una clase o función de este archivo.
    pub fn member(&self, member: &str) -> Option<String> {
        let prefix = self
            .path
            .as_ref()
            .map(|path| path.join("."))
            .or_else(|| self.name.clone())?;
        Some(format!("{prefix}{IMPORT_SEPARATOR}{member}"))
    }

    // `from_relative_import`: cada punto saca un segmento del módulo. Sin la ruta del paquete, o si se
    // pasa de la raíz, no resuelve a nada.
    pub fn resolved_import(&self, qualified: &str) -> Option<String> {
        let relative = qualified.trim_start_matches(IMPORT_SEPARATOR);
        let level = qualified.len().saturating_sub(relative.len());
        if level == 0 {
            return Some(qualified.to_owned());
        }
        let path = self.path.as_ref()?;
        let kept = path.len().checked_sub(level)?;
        let mut segments: Vec<&str> = path.iter().take(kept).map(String::as_str).collect();
        segments.extend(relative.split(IMPORT_SEPARATOR));
        Some(segments.join("."))
    }
}

// `to_module_path`: cada componente desde el padre del paquete, sin extensión; un `__init__.py` deja
// también `__init__`, como en ruff.
fn to_module_path(package: &Path, file: &Path) -> Option<Vec<String>> {
    file.strip_prefix(package.parent()?)
        .ok()?
        .iter()
        .map(|part| {
            Path::new(part)
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
        })
        .collect()
}

#[cfg(test)]
mod tests;
