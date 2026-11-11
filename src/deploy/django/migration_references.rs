use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::discovery::apps;
use super::factories::module_file;
use super::settings_module::python_files;

const MIGRATIONS_DIRECTORY: &str = "migrations";
const MODULE_SEPARATOR: char = '.';

fn referenced_modules(migration: &str) -> impl Iterator<Item = &str> {
    migration
        .split(|character: char| {
            !(character.is_alphanumeric() || character == '_' || character == MODULE_SEPARATOR)
        })
        .filter_map(|token| {
            let (module, name) = token.rsplit_once(MODULE_SEPARATOR)?;
            let names_a_class = name.starts_with(|first: char| first.is_ascii_uppercase());
            let is_module = module.starts_with(|first: char| first.is_ascii_lowercase());
            (names_a_class && is_module).then_some(module)
        })
}

pub fn modules_named_by_migrations(source_root: &Path) -> BTreeMap<PathBuf, String> {
    let sources: Vec<String> = apps(source_root)
        .iter()
        .flat_map(|app| python_files(&app.directory.join(MIGRATIONS_DIRECTORY)))
        .filter_map(|migration| fs::read_to_string(migration).ok())
        .collect();
    let mut named = BTreeMap::new();
    for module in sources.iter().flat_map(|source| referenced_modules(source)) {
        if let Some(file) = module_file(source_root, module) {
            named.entry(file).or_insert_with(|| module.to_owned());
        }
    }
    named
}

#[cfg(test)]
mod tests {
    use super::super::model::Class;
    use super::super::project_fields::field_classes;
    use super::*;

    const PACKAGE_MARKER: &str = "__init__.py";

    #[test]
    fn a_field_declared_with_the_models_counts_when_a_migration_names_its_module() {
        let project = std::env::temp_dir().join(format!("kerbo-django-model-fields-{}", std::process::id()));
        let app = project.join("users");
        let migrations = app.join(MIGRATIONS_DIRECTORY);
        fs::create_dir_all(&migrations).expect("se crea la app de prueba");
        fs::write(app.join(PACKAGE_MARKER), "").expect("se marca el paquete");
        fs::write(migrations.join(PACKAGE_MARKER), "").expect("se marcan las migraciones");
        fs::write(
            app.join("models.py"),
            "class AuthenticatorConfig(models.JSONField):
    pass
",
        )
        .expect("se escriben los modelos");
        fs::write(
            migrations.join("0001_initial.py"),
            "fields=[(\"config\", users.models.AuthenticatorConfig())]
",
        )
        .expect("se escribe la migración");
        let classes = field_classes(&project);
        fs::remove_dir_all(&project).expect("se borra el proyecto de prueba");
        assert_eq!(
            classes.get("users.models.AuthenticatorConfig"),
            Some(&Class::Json)
        );
    }
}
