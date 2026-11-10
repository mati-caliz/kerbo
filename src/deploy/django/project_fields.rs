use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::discovery::subdirectories;
use super::fields::class_of;
use super::model::Class;

const FIELDS_MODULE: &str = "fields.py";
const FIELDS_PACKAGE: &str = "fields";
const PACKAGE_MARKER: &str = "__init__.py";
const PYTHON_EXTENSION: &str = "py";
const SKIPPED_DIRECTORIES: [&str; 2] = ["migrations", "node_modules"];
const CLASS_KEYWORD: &str = "class ";
const FUNCTION_KEYWORD: &str = "def ";
const COLUMN_TYPE_METHOD: &str = "def db_type";
const RETURN_KEYWORD: &str = "return ";
const COMMENT: char = '#';
const BASES_OPENER: char = '(';
const BASES_CLOSER: char = ')';
const SUBSCRIPT_OPENER: char = '[';
const SUBSCRIPT_CLOSER: char = ']';
const BASE_SEPARATOR: char = ',';
const KEYWORD_ARGUMENT: char = '=';
const ALIAS_SEPARATOR: &str = " = ";
const MODULE_SEPARATOR: char = '.';
const QUOTES: [char; 2] = ['"', '\''];
const COLUMN_TYPES: &[(&str, Class)] = &[
    ("text", Class::Text),
    ("uuid", Class::Uuid),
    ("jsonb", Class::Json),
    ("bigint", Class::BigInteger),
    ("integer", Class::Integer),
    ("smallint", Class::SmallInteger),
    ("boolean", Class::Boolean),
    ("bytea", Class::Binary),
    ("inet", Class::Inet),
    ("date", Class::Date),
    ("timestamp with time zone", Class::DateTime),
];

struct Declaration {
    qualified: String,
    name: String,
    bases: Vec<String>,
    column_type: Option<Class>,
}

fn fields_modules(directory: &Path, inside_fields_package: bool, found: &mut Vec<PathBuf>) {
    let modules = fs::read_dir(directory)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.is_file())
                .filter(|path| {
                    if inside_fields_package {
                        path.extension()
                            .is_some_and(|extension| extension == PYTHON_EXTENSION)
                    } else {
                        path.file_name().is_some_and(|name| name == FIELDS_MODULE)
                    }
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    found.extend(modules);
    for child in subdirectories(directory) {
        let name = child
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if SKIPPED_DIRECTORIES.contains(&name) {
            continue;
        }
        fields_modules(&child, inside_fields_package || name == FIELDS_PACKAGE, found);
    }
}

fn module_path(source_root: &Path, module: &Path) -> String {
    let relative = module.strip_prefix(source_root).unwrap_or(module);
    let package = module.file_name().is_some_and(|name| name == PACKAGE_MARKER);
    let without_file = if package {
        relative.parent().unwrap_or(relative)
    } else {
        relative
    };
    without_file
        .with_extension("")
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect::<Vec<_>>()
        .join(&MODULE_SEPARATOR.to_string())
}

fn without_comment(line: &str) -> &str {
    line.split_once(COMMENT).map_or(line, |(code, _)| code).trim_end()
}

fn closing_of(text: &str, opener: char, closer: char) -> Option<usize> {
    let mut depth = 0usize;
    for (index, character) in text.char_indices() {
        if character == opener {
            depth += 1;
        } else if character == closer {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(index);
            }
        }
    }
    None
}

fn top_level_parts(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;
    for (index, character) in text.char_indices() {
        match character {
            SUBSCRIPT_OPENER | BASES_OPENER => depth += 1,
            SUBSCRIPT_CLOSER | BASES_CLOSER => depth = depth.saturating_sub(1),
            BASE_SEPARATOR if depth == 0 => {
                parts.push(&text[start..index]);
                start = index + BASE_SEPARATOR.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(&text[start..]);
    parts
}

fn bare_name(expression: &str) -> &str {
    let unsubscripted = expression
        .split(SUBSCRIPT_OPENER)
        .next()
        .unwrap_or(expression)
        .trim();
    unsubscripted
        .rsplit(MODULE_SEPARATOR)
        .next()
        .unwrap_or(unsubscripted)
}

fn class_header(line: &str) -> Option<(String, Vec<String>)> {
    let rest = without_comment(line).strip_prefix(CLASS_KEYWORD)?;
    let name_end = rest.find([BASES_OPENER, SUBSCRIPT_OPENER, ':'])?;
    let name = rest[..name_end].trim().to_owned();
    let mut after_name = &rest[name_end..];
    if after_name.starts_with(SUBSCRIPT_OPENER) {
        after_name = &after_name[closing_of(after_name, SUBSCRIPT_OPENER, SUBSCRIPT_CLOSER)? + 1..];
    }
    let bases = if after_name.starts_with(BASES_OPENER) {
        let closing = closing_of(after_name, BASES_OPENER, BASES_CLOSER)?;
        top_level_parts(&after_name[1..closing])
            .into_iter()
            .filter(|base| !base.contains(KEYWORD_ARGUMENT) && !base.trim().is_empty())
            .map(|base| bare_name(base).to_owned())
            .collect()
    } else {
        Vec::new()
    };
    Some((name, bases))
}

fn alias(line: &str) -> Option<(String, String)> {
    let (alias, target) = without_comment(line).trim().split_once(ALIAS_SEPARATOR)?;
    let is_identifier = |text: &str| {
        !text.is_empty()
            && text
                .chars()
                .all(|character| character.is_alphanumeric() || character == '_')
    };
    let target = bare_name(target);
    (is_identifier(alias) && is_identifier(target) && alias != target)
        .then(|| (alias.to_owned(), target.to_owned()))
}

fn returned_column_type(line: &str) -> Option<Class> {
    let literal = without_comment(line).trim().strip_prefix(RETURN_KEYWORD)?.trim();
    let column_type = literal.strip_prefix(QUOTES)?.strip_suffix(QUOTES)?;
    COLUMN_TYPES
        .iter()
        .find(|(name, _)| *name == column_type)
        .map(|(_, class)| *class)
}

fn declarations(source: &str, module: &str) -> Vec<Declaration> {
    let aliases: BTreeMap<String, String> = source.lines().filter_map(alias).collect();
    let mut declared: Vec<Declaration> = Vec::new();
    let mut inside_column_type = false;
    for line in source.lines() {
        if let Some((name, bases)) = class_header(line) {
            declared.push(Declaration {
                qualified: format!("{module}{MODULE_SEPARATOR}{name}"),
                name,
                bases: bases
                    .into_iter()
                    .map(|base| aliases.get(&base).cloned().unwrap_or(base))
                    .collect(),
                column_type: None,
            });
            inside_column_type = false;
            continue;
        }
        let code = without_comment(line);
        if code.is_empty() {
            continue;
        }
        if !code.starts_with(char::is_whitespace) {
            inside_column_type = false;
            continue;
        }
        let trimmed = code.trim_start();
        if trimmed.starts_with(FUNCTION_KEYWORD) {
            inside_column_type = trimmed.starts_with(COLUMN_TYPE_METHOD);
        } else if inside_column_type
            && let Some(class) = returned_column_type(code)
            && let Some(current) = declared.last_mut()
        {
            current.column_type.get_or_insert(class);
        }
    }
    declared
}

fn base_class(base: &str, by_name: &BTreeMap<String, Class>) -> Option<Class> {
    Some(class_of(base))
        .filter(|known| *known != Class::Other)
        .or_else(|| by_name.get(base).copied())
}

pub fn field_classes(source_root: &Path) -> BTreeMap<String, Class> {
    let mut modules = Vec::new();
    fields_modules(source_root, false, &mut modules);
    modules.sort();
    let declared: Vec<Declaration> = modules
        .iter()
        .filter_map(|module| {
            let source = fs::read_to_string(module).ok()?;
            Some(declarations(&source, &module_path(source_root, module)))
        })
        .flatten()
        .collect();
    let mut by_name: BTreeMap<String, Class> = BTreeMap::new();
    let mut classes = BTreeMap::new();
    for _ in 0..=declared.len() {
        let mut changed = false;
        for declaration in &declared {
            let Some(class) = declaration.column_type.or_else(|| {
                declaration
                    .bases
                    .iter()
                    .find_map(|base| base_class(base, &by_name))
            }) else {
                continue;
            };
            if classes.insert(declaration.qualified.clone(), class) != Some(class) {
                changed = true;
            }
            if class_of(&declaration.name) == Class::Other && !by_name.contains_key(&declaration.name) {
                by_name.insert(declaration.name.clone(), class);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    classes.extend(by_name);
    classes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_project_field_takes_the_class_of_the_django_field_it_extends() {
        let source = "from django.db import models\n\n\nclass Shifted(str):\n    pass\n\n\nclass ShiftedField(models.IntegerField):\n    pass\n\n\nclass DoubleShiftedField(ShiftedField):\n    pass\n";
        let declared = declarations(source, "accounts.fields");
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
        assert_eq!(
            classes.get("accounts.fields.DoubleShiftedField"),
            Some(&Class::Integer)
        );
        assert!(!classes.contains_key("Shifted"));
        assert!(!classes.contains_key("Ignored"));
    }

    #[test]
    fn reads_fields_packages_with_generic_bases_mixins_aliases_and_column_types() {
        let project =
            std::env::temp_dir().join(format!("kerbo-django-fields-package-{}", std::process::id()));
        let fields = project.join("shop").join("db").join("fields");
        fs::create_dir_all(fields.join("encryption")).expect("se crea el paquete de campos");
        fs::write(
            fields.join("bounded.py"),
            "class BoundedBigAutoField(models.BigAutoField):  # type: ignore[type-arg]\n    MAX_VALUE = 1\n\n\nclass FlexibleForeignKey(ForeignKey[SetType, GetType]):\n    pass\n",
        )
        .expect("se escriben los campos acotados");
        fs::write(
            fields.join("text.py"),
            "class TextType:\n    def db_type(self, connection) -> str:\n        return \"text\"\n\n\nclass CharField[GetT: str | None](TextType, models.CharField[GetT, GetT]):\n    pass\n",
        )
        .expect("se escriben los campos de texto");
        fs::write(
            fields.join("jsonfield.py"),
            "class JSONField(models.TextField[Any, Any]):\n    pass\n\n\nclass LegacyTextJSONField(models.JSONField):\n    def db_type(self, connection):\n        # usually jsonb\n        return \"text\"\n",
        )
        .expect("se escriben los campos JSON");
        fs::write(
            fields.join("slug.py"),
            "if TYPE_CHECKING:\n    _SlugField = SlugField[str, str]\nelse:\n    _SlugField = SlugField\n\n\nclass ShopSlugField(_SlugField):\n    pass\n",
        )
        .expect("se escriben los slugs");
        fs::write(
            fields.join("encryption").join("json.py"),
            "class EncryptedField(Field[ST, GT]):\n    pass\n\n\nclass EncryptedJSONField(EncryptedField[Any, Any], JSONField[Any, Any]):\n    pass\n",
        )
        .expect("se escriben los campos cifrados");
        let classes = field_classes(&project);
        fs::remove_dir_all(&project).expect("se borra el proyecto de prueba");
        assert_eq!(classes.get("BoundedBigAutoField"), Some(&Class::BigAuto));
        assert_eq!(classes.get("FlexibleForeignKey"), Some(&Class::ForeignKey));
        assert_eq!(classes.get("shop.db.fields.text.CharField"), Some(&Class::Text));
        assert_eq!(
            classes.get("shop.db.fields.jsonfield.JSONField"),
            Some(&Class::Text)
        );
        assert_eq!(classes.get("LegacyTextJSONField"), Some(&Class::Text));
        assert_eq!(classes.get("ShopSlugField"), Some(&Class::Slug));
        assert_eq!(classes.get("EncryptedJSONField"), Some(&Class::Json));
        assert!(!classes.contains_key("CharField"));
        assert!(!classes.contains_key("EncryptedField"));
    }
}
