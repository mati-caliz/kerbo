use std::path::{Path, PathBuf};

use super::stdlib::STANDARD_LIBRARY;

const FUTURE: &str = "__future__";
const MAIN: &str = "__main__";
const SEPARATOR: char = '.';
const SOURCE_ROOTS: [&str; 2] = [".", "src"];
const MODULE_EXTENSIONS: [&str; 2] = ["py", "pyi"];
const PACKAGE_MARKER: &str = "__init__.py";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Section {
    Future,
    StandardLibrary,
    ThirdParty,
    FirstParty,
    LocalFolder,
}

pub const SECTION_ORDER: [Section; 5] = [
    Section::Future,
    Section::StandardLibrary,
    Section::ThirdParty,
    Section::FirstParty,
    Section::LocalFolder,
];

// Dónde está el archivo: el paquete al que pertenece y la carpeta desde la que se corre, que con la
// configuración por defecto de ruff es la raíz del proyecto (`src = [".", "src"]`).
#[derive(Debug, Clone, Default)]
pub struct Location {
    pub package: Option<PathBuf>,
}

impl Location {
    pub fn of(path: Option<&Path>) -> Self {
        Self {
            package: path.and_then(Path::parent).and_then(package_root),
        }
    }

    // `categorize` de ruff con la configuración por defecto.
    pub fn section(&self, module: &str, is_relative: bool) -> Section {
        let base = module.split(SEPARATOR).next().unwrap_or_default();
        if !is_relative && base == FUTURE {
            Section::Future
        } else if is_relative {
            Section::LocalFolder
        } else if STANDARD_LIBRARY.binary_search(&base).is_ok() {
            Section::StandardLibrary
        } else if self.is_same_package(base) || matches_sources(module) || module == MAIN {
            Section::FirstParty
        } else {
            Section::ThirdParty
        }
    }

    fn is_same_package(&self, base: &str) -> bool {
        self.package
            .as_deref()
            .and_then(Path::file_name)
            .is_some_and(|name| name == base)
    }
}

// `detect_package_root`: la carpeta más alta de la cadena de paquetes que contiene al archivo.
pub fn package_root(directory: &Path) -> Option<PathBuf> {
    let mut root = None;
    for ancestor in directory.ancestors() {
        if ancestor.as_os_str().is_empty() || !is_package(ancestor) {
            break;
        }
        root = Some(ancestor.to_path_buf());
    }
    root
}

fn is_package(directory: &Path) -> bool {
    directory.join(PACKAGE_MARKER).is_file()
}

// `match_sources`: el módulo existe como carpeta o archivo bajo alguna raíz de código.
fn matches_sources(module: &str) -> bool {
    let relative: PathBuf = module.split(SEPARATOR).collect();
    if relative.as_os_str().is_empty() {
        return false;
    }
    SOURCE_ROOTS.iter().any(|root| {
        let candidate = Path::new(root).join(&relative);
        candidate.is_dir()
            || MODULE_EXTENSIONS
                .iter()
                .any(|extension| candidate.with_extension(extension).is_file())
    })
}
