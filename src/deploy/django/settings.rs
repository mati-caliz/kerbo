use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, PoisonError};

use super::django_version::{Version, project_version};
use super::fields::class_of;
use super::installed_apps::installed_apps;
use super::model::{Class, ModelKey};

const SETTINGS_MODULE: &str = "settings";
const PYTHON_EXTENSION: &str = "py";
const USER_MODEL_SETTING: &str = "AUTH_USER_MODEL";
const AUTO_FIELD_SETTING: &str = "DEFAULT_AUTO_FIELD";
const DEFAULT_USER_APP: &str = "auth";
const DEFAULT_USER_MODEL: &str = "user";
const APP_SEPARATOR: char = '.';
const ASSIGNMENT: char = '=';
const QUOTES: &[char] = &['"', '\''];
const SETTINGS_PREFIX: &str = "settings.";
const CASCADE_DROPPED_IN: Version = Version::new(6, 0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub user_model: ModelKey,
    pub auto_field: Class,
    pub numbers: BTreeMap<String, String>,
    pub installed_apps: Option<BTreeSet<String>>,
    pub column_drops_cascade: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            user_model: ModelKey::new(DEFAULT_USER_APP, DEFAULT_USER_MODEL),
            auto_field: Class::Auto,
            numbers: BTreeMap::new(),
            installed_apps: None,
            column_drops_cascade: false,
        }
    }
}

impl Settings {
    fn of_a_new_project() -> Self {
        Self {
            auto_field: Class::BigAuto,
            ..Self::default()
        }
    }

    fn read(sources: &[String]) -> Self {
        let mut settings = Self::default();
        if let Some(model) = assigned(sources, USER_MODEL_SETTING).and_then(|value| {
            value
                .split_once(APP_SEPARATOR)
                .map(|(app, model)| ModelKey::new(app, model))
        }) {
            settings.user_model = model;
        }
        if let Some(class) = assigned(sources, AUTO_FIELD_SETTING)
            .and_then(|value| value.rsplit(APP_SEPARATOR).next().map(class_of))
            .filter(|class| *class != Class::Other)
        {
            settings.auto_field = class;
        }
        settings.numbers = numbers(sources);
        settings.installed_apps = installed_apps(sources);
        settings
    }

    pub fn replacement(&self, setting: &str, model: &ModelKey) -> Option<ModelKey> {
        (setting == USER_MODEL_SETTING && self.user_model != *model).then(|| self.user_model.clone())
    }

    pub fn includes_app(&self, app: &str) -> bool {
        self.installed_apps.as_ref().is_none_or(|apps| apps.contains(app))
    }

    pub fn number<'value>(&'value self, reference: &'value str) -> &'value str {
        reference
            .strip_prefix(SETTINGS_PREFIX)
            .and_then(|name| self.numbers.get(name))
            .map_or(reference, String::as_str)
    }
}

fn is_constant_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|character| character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_')
}

fn is_integer(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|character| character.is_ascii_digit())
}

fn numbers(sources: &[String]) -> BTreeMap<String, String> {
    sources
        .iter()
        .flat_map(|source| source.lines())
        .filter_map(|line| line.split_once(ASSIGNMENT))
        .map(|(name, value)| (name.trim_end(), value.trim()))
        .filter(|(name, value)| is_constant_name(name) && is_integer(value))
        .map(|(name, value)| (name.to_owned(), value.to_owned()))
        .collect()
}

fn assigned(sources: &[String], name: &str) -> Option<String> {
    sources
        .iter()
        .flat_map(|source| source.lines())
        .filter_map(|line| line.strip_prefix(name))
        .filter_map(|rest| rest.trim_start().strip_prefix(ASSIGNMENT))
        .map(str::trim)
        .find_map(|value| {
            let unquoted = value.strip_prefix(QUOTES)?.strip_suffix(QUOTES)?;
            Some(unquoted.to_owned())
        })
}

fn candidates(apps_root: &Path) -> Vec<PathBuf> {
    let mut packages = vec![apps_root.to_path_buf()];
    if let Ok(entries) = fs::read_dir(apps_root) {
        packages.extend(
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.is_dir()),
        );
    }
    let mut files = Vec::new();
    for package in packages {
        files.push(package.join(SETTINGS_MODULE).with_extension(PYTHON_EXTENSION));
        if let Ok(entries) = fs::read_dir(package.join(SETTINGS_MODULE)) {
            files.extend(
                entries
                    .filter_map(Result::ok)
                    .map(|entry| entry.path())
                    .filter(|path| {
                        path.extension()
                            .is_some_and(|extension| extension == PYTHON_EXTENSION)
                    }),
            );
        }
    }
    files.sort();
    files
}

fn discovered(apps_root: &Path) -> Settings {
    let sources: Vec<String> = candidates(apps_root)
        .iter()
        .filter_map(|path| fs::read_to_string(path).ok())
        .collect();
    let mut settings = if sources.is_empty() {
        Settings::of_a_new_project()
    } else {
        Settings::read(&sources)
    };
    settings.column_drops_cascade =
        project_version(apps_root).is_some_and(|version| version < CASCADE_DROPPED_IN);
    settings
}

pub fn project(apps_root: &Path) -> Settings {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Settings>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Mutex::default);
    if let Some(known) = cache
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(apps_root)
    {
        return known.clone();
    }
    let settings = discovered(apps_root);
    cache
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(apps_root.to_path_buf(), settings.clone());
    settings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_user_model_the_automatic_key_and_the_numeric_constants() {
        let sources = vec![
            "import os
AUTH_USER_MODEL = \"account.User\"
DEFAULT_AUTO_FIELD = 'django.db.models.BigAutoField'
CODE_LENGTH = 3
    INDENTED = 4
"
            .to_owned(),
        ];
        let settings = Settings::read(&sources);
        assert_eq!(settings.user_model, ModelKey::new("account", "user"));
        assert_eq!(settings.auto_field, Class::BigAuto);
        assert_eq!(settings.number("settings.CODE_LENGTH"), "3");
        assert_eq!(settings.number("settings.INDENTED"), "settings.INDENTED");
        assert_eq!(settings.number("CODE_LENGTH"), "CODE_LENGTH");
    }

    #[test]
    fn only_the_configured_user_model_survives_its_swappable_setting() {
        let settings = Settings::default();
        assert_eq!(
            settings.replacement("AUTH_USER_MODEL", &ModelKey::new("auth", "user")),
            None
        );
        assert_eq!(
            settings.replacement("AUTH_USER_MODEL", &ModelKey::new("account", "user")),
            Some(ModelKey::new("auth", "user"))
        );
        assert_eq!(
            settings.replacement("OTHER_MODEL", &ModelKey::new("account", "user")),
            None
        );
    }

    #[test]
    fn falls_back_to_the_django_defaults() {
        let sources = vec!["AUTH_USER_MODEL = get_user_model_name()\n".to_owned()];
        assert_eq!(Settings::read(&sources), Settings::default());
    }

    #[test]
    fn without_settings_files_assumes_the_project_template() {
        let settings = discovered(Path::new("no-existe"));
        assert_eq!(settings.auto_field, Class::BigAuto);
        assert!(!settings.column_drops_cascade);
    }

    #[test]
    fn a_project_locked_before_django_six_drops_columns_in_cascade() {
        let project = std::env::temp_dir().join(format!("kerbo-django-cascade-{}", std::process::id()));
        let apps_root = project.join("apps");
        fs::create_dir_all(&apps_root).expect("se crea el proyecto de prueba");
        fs::write(
            project.join("uv.lock"),
            "[[package]]\nname = \"django\"\nversion = \"5.2.19\"\n",
        )
        .expect("se escribe el lock");
        let settings = discovered(&apps_root);
        fs::remove_dir_all(&project).expect("se borra el proyecto de prueba");
        assert!(settings.column_drops_cascade);
    }
}
