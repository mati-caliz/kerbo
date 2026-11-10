use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use super::discovery::subdirectories;

const SETTINGS_VARIABLE: &str = "DJANGO_SETTINGS_MODULE";
const PYTHON_EXTENSION: &str = "py";
const PACKAGE_MARKER: &str = "__init__.py";
const MODULE_SEPARATOR: char = '.';
const QUOTES: [char; 2] = ['"', '\''];

pub fn declared_modules(source_root: &Path) -> Vec<PathBuf> {
    let mut seen = BTreeSet::new();
    python_files(source_root)
        .iter()
        .filter_map(|file| fs::read_to_string(file).ok())
        .flat_map(|source| named_modules(&source))
        .filter(|module| seen.insert(module.clone()))
        .filter_map(|module| module_file(source_root, &module))
        .collect()
}

fn python_files(directory: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(directory)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.is_file())
                .filter(|path| {
                    path.extension()
                        .is_some_and(|extension| extension == PYTHON_EXTENSION)
                })
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    for child in subdirectories(directory) {
        files.extend(python_files(&child));
    }
    files
}

fn named_modules(source: &str) -> Vec<String> {
    source
        .lines()
        .filter_map(|line| {
            let after_variable = line.split_once(SETTINGS_VARIABLE)?.1;
            let after_closing_quote = after_variable.strip_prefix(QUOTES)?;
            let opening = after_closing_quote.find(QUOTES)?;
            let literal = &after_closing_quote[opening + 1..];
            let module = &literal[..literal.find(QUOTES)?];
            is_dotted_name(module).then(|| module.to_owned())
        })
        .collect()
}

fn is_dotted_name(module: &str) -> bool {
    module.split(MODULE_SEPARATOR).all(|segment| {
        !segment.is_empty()
            && segment
                .chars()
                .all(|character| character.is_alphanumeric() || character == '_')
    })
}

fn module_file(source_root: &Path, module: &str) -> Option<PathBuf> {
    let path = module
        .split(MODULE_SEPARATOR)
        .fold(source_root.to_path_buf(), |path, segment| path.join(segment));
    let file = path.with_extension(PYTHON_EXTENSION);
    let package = path.join(PACKAGE_MARKER);
    [file, package].into_iter().find(|candidate| candidate.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_module_of_every_way_of_declaring_it() {
        let source = "os.environ.setdefault(\"DJANGO_SETTINGS_MODULE\", \"shop.conf.server\")
os.environ['DJANGO_SETTINGS_MODULE'] = 'shop.settings'
if os.environ.get(\"DJANGO_SETTINGS_MODULE\"):
    pass
";
        assert_eq!(named_modules(source), ["shop.conf.server", "shop.settings"]);
    }

    #[test]
    fn resolves_modules_and_packages_under_the_source_root() {
        let root = std::env::temp_dir().join(format!("kerbo-django-settings-module-{}", std::process::id()));
        let conf = root.join("shop").join("conf");
        fs::create_dir_all(conf.join("production")).expect("se crea el proyecto de prueba");
        fs::write(conf.join("server.py"), "").expect("se escribe el módulo");
        fs::write(conf.join("production").join(PACKAGE_MARKER), "").expect("se escribe el paquete");
        fs::write(
            root.join("shop").join("runner.py"),
            "os.environ.setdefault(\"DJANGO_SETTINGS_MODULE\", \"shop.conf.server\")\nos.environ[\"DJANGO_SETTINGS_MODULE\"] = \"shop.conf.production\"\nos.environ[\"DJANGO_SETTINGS_MODULE\"] = \"shop.missing\"\n",
        )
        .expect("se escribe quien declara los settings");
        let found = declared_modules(&root);
        fs::remove_dir_all(&root).expect("se borra el proyecto de prueba");
        assert_eq!(
            found,
            [
                conf.join("server.py"),
                conf.join("production").join(PACKAGE_MARKER)
            ]
        );
    }
}
