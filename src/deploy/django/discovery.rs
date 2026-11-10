use std::collections::{BTreeSet, HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

const MIGRATIONS_DIRECTORY: &str = "migrations";
const PACKAGE_MARKER: &str = "__init__.py";
const CONFIG_MODULE: &str = "apps.py";
const LABEL_ATTRIBUTE: &str = "label";
const ASSIGNMENT: char = '=';
const QUOTES: &[char] = &['"', '\''];
const HIDDEN_PREFIX: char = '.';

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct App {
    pub label: String,
    pub directory: PathBuf,
}

impl App {
    pub fn directory_name(&self) -> String {
        directory_name(&self.directory)
    }
}

fn directory_name(directory: &Path) -> String {
    directory
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

pub fn is_package(directory: &Path) -> bool {
    directory.join(PACKAGE_MARKER).is_file()
}

pub fn source_root(app_directory: &Path) -> PathBuf {
    let mut current = app_directory;
    if !is_package(current) {
        match current.parent() {
            Some(parent) => current = parent,
            None => return PathBuf::new(),
        }
    }
    while is_package(current) {
        match current.parent() {
            Some(parent) => current = parent,
            None => break,
        }
    }
    current.to_path_buf()
}

pub fn subdirectories(directory: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .filter(|path| !directory_name(path).starts_with(HIDDEN_PREFIX))
        .collect();
    found.sort();
    found
}

fn label(app_directory: &Path) -> String {
    fs::read_to_string(app_directory.join(CONFIG_MODULE))
        .ok()
        .and_then(|source| {
            source.lines().find_map(|line| {
                let value = line
                    .trim()
                    .strip_prefix(LABEL_ATTRIBUTE)?
                    .trim_start()
                    .strip_prefix(ASSIGNMENT)?
                    .trim();
                Some(value.strip_prefix(QUOTES)?.strip_suffix(QUOTES)?.to_owned())
            })
        })
        .unwrap_or_else(|| directory_name(app_directory))
}

fn scanned(source_root: &Path) -> Vec<App> {
    let mut apps = Vec::new();
    let mut labels = BTreeSet::new();
    let mut pending = VecDeque::from([(source_root.to_path_buf(), true)]);
    while let Some((directory, is_root)) = pending.pop_front() {
        for child in subdirectories(&directory) {
            if directory_name(&child) == MIGRATIONS_DIRECTORY {
                continue;
            }
            let package = is_package(&child);
            let configured = package || is_root || child.join(CONFIG_MODULE).is_file();
            let app = (configured && child.join(MIGRATIONS_DIRECTORY).is_dir()).then(|| App {
                label: label(&child),
                directory: child.clone(),
            });
            apps.extend(app.filter(|found| labels.insert(found.label.clone())));
            if package {
                pending.push_back((child, false));
            }
        }
    }
    apps
}

pub fn apps(source_root: &Path) -> Arc<[App]> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Arc<[App]>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Mutex::default);
    if let Some(known) = cache
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(source_root)
    {
        return Arc::clone(known);
    }
    let found: Arc<[App]> = Arc::from(scanned(source_root));
    cache
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(source_root.to_path_buf(), Arc::clone(&found));
    found
}

pub fn app_label(source_root: &Path, app_directory: &Path) -> String {
    apps(source_root)
        .iter()
        .find(|app| app.directory == app_directory)
        .map_or_else(|| directory_name(app_directory), |app| app.label.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package(directory: &Path) {
        fs::create_dir_all(directory.join(MIGRATIONS_DIRECTORY)).expect("se crea la app de prueba");
        fs::write(directory.join(PACKAGE_MARKER), "").expect("se marca el paquete");
    }

    #[test]
    fn finds_nested_apps_by_their_configured_label() {
        let project = std::env::temp_dir().join(format!("kerbo-django-labels-{}", std::process::id()));
        let core = project.join("cms");
        let images = core.join("images");
        let forms = core.join("contrib").join("forms");
        package(&core);
        package(&images);
        package(&forms);
        fs::write(core.join("contrib").join(PACKAGE_MARKER), "").expect("se marca contrib");
        fs::write(
            images.join(CONFIG_MODULE),
            "class Images(AppConfig):\n    label = \"cmsimages\"\n",
        )
        .expect("se escribe la configuración");
        let root = source_root(&images);
        let labels: Vec<String> = scanned(&root).into_iter().map(|app| app.label).collect();
        let core_label = app_label(&root, &core);
        fs::remove_dir_all(&project).expect("se borra el proyecto de prueba");
        assert_eq!(root, project);
        assert_eq!(labels, ["cms", "cmsimages", "forms"]);
        assert_eq!(core_label, "cms");
    }

    #[test]
    fn an_app_without_package_marker_inside_a_package_belongs_to_the_whole_project() {
        let project = std::env::temp_dir().join(format!("kerbo-django-namespace-{}", std::process::id()));
        let core = project.join("cms");
        let tests = core.join("tests");
        let streams = tests.join("streams");
        package(&core);
        package(&tests);
        fs::create_dir_all(streams.join(MIGRATIONS_DIRECTORY)).expect("se crean las migraciones");
        fs::write(
            streams.join(CONFIG_MODULE),
            "class Streams(AppConfig):
    label = \"stream_tests\"
",
        )
        .expect("se escribe la configuración");
        let root = source_root(&streams);
        let label = app_label(&root, &streams);
        fs::remove_dir_all(&project).expect("se borra el proyecto de prueba");
        assert_eq!(root, project);
        assert_eq!(label, "stream_tests");
    }

    #[test]
    fn an_app_outside_a_package_hangs_from_its_parent() {
        assert_eq!(source_root(Path::new("proyecto/blog")), PathBuf::from("proyecto"));
    }
}
