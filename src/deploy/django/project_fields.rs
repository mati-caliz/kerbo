use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::discovery::subdirectories;
use super::fields::class_of;
use super::model::Class;

const FIELDS_MODULE: &str = "fields.py";
const SKIPPED_DIRECTORIES: [&str; 2] = ["migrations", "node_modules"];
const CLASS_KEYWORD: &str = "class ";
const BASES_OPENER: char = '(';
const BASES_CLOSER: &str = "):";
const MODULE_SEPARATOR: char = '.';

fn fields_modules(source_root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![source_root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let module = directory.join(FIELDS_MODULE);
        if module.is_file() {
            found.push(module);
        }
        pending.extend(subdirectories(&directory).into_iter().filter(|child| {
            child
                .file_name()
                .is_none_or(|name| !SKIPPED_DIRECTORIES.iter().any(|skipped| name == *skipped))
        }));
    }
    found.sort();
    found
}

fn declarations(source: &str) -> Vec<(String, String)> {
    source
        .lines()
        .filter_map(|line| {
            let (name, rest) = line.strip_prefix(CLASS_KEYWORD)?.split_once(BASES_OPENER)?;
            let base = rest.strip_suffix(BASES_CLOSER)?.trim();
            let base = base.rsplit(MODULE_SEPARATOR).next().unwrap_or(base);
            Some((name.trim().to_owned(), base.to_owned()))
        })
        .collect()
}

pub fn field_classes(source_root: &Path) -> BTreeMap<String, Class> {
    let declared: Vec<(String, String)> = fields_modules(source_root)
        .iter()
        .filter_map(|module| fs::read_to_string(module).ok())
        .flat_map(|source| declarations(&source))
        .collect();
    let mut classes = BTreeMap::new();
    let mut changed = true;
    while changed {
        changed = false;
        for (name, base) in &declared {
            if classes.contains_key(name) || class_of(name) != Class::Other {
                continue;
            }
            let class = Some(class_of(base))
                .filter(|known| *known != Class::Other)
                .or_else(|| classes.get(base).copied());
            if let Some(class) = class {
                classes.insert(name.clone(), class);
                changed = true;
            }
        }
    }
    classes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_project_field_takes_the_class_of_the_django_field_it_extends() {
        let source = "from django.db import models\n\n\nclass Shifted(str):\n    pass\n\n\nclass ShiftedField(models.IntegerField):\n    pass\n\n\nclass DoubleShiftedField(ShiftedField):\n    pass\n";
        let declared = declarations(source);
        assert_eq!(declared.len(), 3);
        let project = std::env::temp_dir().join(format!("kerbo-django-fields-{}", std::process::id()));
        let app = project.join("accounts");
        fs::create_dir_all(app.join("migrations")).expect("se crea la app de prueba");
        fs::write(app.join(FIELDS_MODULE), source).expect("se escriben los campos");
        fs::write(
            app.join("migrations").join(FIELDS_MODULE),
            "class Ignored(models.TextField):\n",
        )
        .expect("se escribe un módulo que no cuenta");
        let classes = field_classes(&project);
        fs::remove_dir_all(&project).expect("se borra el proyecto de prueba");
        assert_eq!(classes.get("ShiftedField"), Some(&Class::Integer));
        assert_eq!(classes.get("DoubleShiftedField"), Some(&Class::Integer));
        assert!(!classes.contains_key("Shifted"));
        assert!(!classes.contains_key("Ignored"));
    }
}
