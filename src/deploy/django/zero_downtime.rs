use std::fs;
use std::path::Path;

use super::factories::module_file;
use super::settings_module::python_files;

const PACKAGE: &str = "django_zero_downtime_migrations";
const ENGINE_KEYS: [&str; 2] = ["\"ENGINE\"", "'ENGINE'"];
const QUOTES: [char; 2] = ['"', '\''];
const KEY_SEPARATOR: char = ':';
const NOT_NULL_SUFFIX: &str = "_notnull";
const DROPS_IF_EXISTS: &str = "DROP CONSTRAINT IF EXISTS";
const PACKAGE_MARKER: &str = "__init__.py";
const ALTER_TABLE: &str = "ALTER TABLE ";
const ADD_CONSTRAINT: &str = " ADD CONSTRAINT ";
const CREATE_INDEX: &str = "CREATE INDEX ";
const CREATE_UNIQUE_INDEX: &str = "CREATE UNIQUE INDEX ";
const CONCURRENTLY: &str = "CONCURRENTLY ";
const DROP_INDEX: &str = "DROP INDEX IF EXISTS ";
const CONCURRENT_DROP_INDEX: &str = "DROP INDEX CONCURRENTLY IF EXISTS ";
const CHECK: &str = "CHECK ";
const FOREIGN_KEY: &str = "FOREIGN KEY ";
const UNIQUE: &str = "UNIQUE";
const PRIMARY_KEY: &str = "PRIMARY KEY ";
const NOT_VALID: &str = " NOT VALID";
const DEFERRABLE: &str = " DEFERRABLE";
const COLUMNS_START: char = '(';

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Backend {
    pub every_migration: bool,
    pub drops_not_null_check: bool,
}

pub fn backend(sources: &[String], source_root: &Path) -> Option<Backend> {
    let engine = sources.iter().find_map(|source| engine(source))?;
    if engine.starts_with(PACKAGE) {
        return Some(Backend {
            every_migration: true,
            drops_not_null_check: false,
        });
    }
    let package = module_file(source_root, &engine)?;
    if package.file_name()? != PACKAGE_MARKER {
        return None;
    }
    let texts: Vec<String> = python_files(package.parent()?)
        .iter()
        .filter_map(|file| fs::read_to_string(file).ok())
        .collect();
    texts.iter().any(|text| text.contains(PACKAGE)).then(|| Backend {
        every_migration: false,
        drops_not_null_check: texts
            .iter()
            .any(|text| text.contains(NOT_NULL_SUFFIX) && text.contains(DROPS_IF_EXISTS)),
    })
}

fn engine(source: &str) -> Option<String> {
    source.lines().find_map(|line| {
        let after_key = ENGINE_KEYS
            .iter()
            .find_map(|key| line.split_once(key))?
            .1
            .trim_start()
            .strip_prefix(KEY_SEPARATOR)?
            .trim_start();
        let quote = after_key
            .chars()
            .next()
            .filter(|character| QUOTES.contains(character))?;
        let literal = &after_key[1..];
        Some(literal[..literal.find(quote)?].to_owned())
    })
}

pub fn translate(sql: &str) -> Vec<String> {
    if let Some(rest) = sql.strip_prefix(CREATE_UNIQUE_INDEX) {
        return vec![concurrent(CREATE_UNIQUE_INDEX, rest)];
    }
    if let Some(rest) = sql.strip_prefix(CREATE_INDEX) {
        return vec![concurrent(CREATE_INDEX, rest)];
    }
    if let Some(rest) = sql.strip_prefix(DROP_INDEX) {
        return vec![format!("{CONCURRENT_DROP_INDEX}{rest}")];
    }
    added_constraint(sql).unwrap_or_else(|| vec![sql.to_owned()])
}

fn concurrent(create: &str, rest: &str) -> String {
    if rest.starts_with(CONCURRENTLY) {
        format!("{create}{rest}")
    } else {
        format!("{create}{CONCURRENTLY}{rest}")
    }
}

fn added_constraint(sql: &str) -> Option<Vec<String>> {
    let (table, constraint) = sql.strip_prefix(ALTER_TABLE)?.split_once(ADD_CONSTRAINT)?;
    let (name, definition) = split_name(constraint)?;
    if definition.starts_with(CHECK) || definition.starts_with(FOREIGN_KEY) {
        return Some(vec![
            format!("{sql}{NOT_VALID}"),
            format!("{ALTER_TABLE}{table} VALIDATE CONSTRAINT {name}"),
        ]);
    }
    let (definition, deferrable) = definition
        .find(DEFERRABLE)
        .map_or((definition, ""), |position| definition.split_at(position));
    let (kind, index) = if let Some(columns) = definition.strip_prefix(PRIMARY_KEY) {
        (PRIMARY_KEY.trim_end(), columns.to_owned())
    } else {
        let unique = definition.strip_prefix(UNIQUE)?;
        let (nulls, columns) = unique.split_at(unique.find(COLUMNS_START)?);
        (UNIQUE, format!("{columns}{}", nulls.trim_end()))
    };
    Some(vec![
        format!("{CREATE_UNIQUE_INDEX}{CONCURRENTLY}{name} ON {table} {index}"),
        format!("{ALTER_TABLE}{table}{ADD_CONSTRAINT}{name} {kind} USING INDEX {name}{deferrable}"),
    ])
}

fn split_name(constraint: &str) -> Option<(&str, &str)> {
    let closing = constraint.strip_prefix('"')?.find('"')? + 2;
    Some((&constraint[..closing], constraint[closing..].trim_start()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_engine_of_the_default_database() {
        let source = "DATABASES = {\"default\": {\n    \"ENGINE\": \"sentry.db.postgres\",\n}}\n";
        assert_eq!(engine(source).as_deref(), Some("sentry.db.postgres"));
    }

    #[test]
    fn makes_indexes_concurrent_and_constraints_validated_apart() {
        assert_eq!(
            translate("CREATE INDEX \"i\" ON \"t\" (\"a\")"),
            ["CREATE INDEX CONCURRENTLY \"i\" ON \"t\" (\"a\")"]
        );
        assert_eq!(
            translate("DROP INDEX IF EXISTS \"i\""),
            ["DROP INDEX CONCURRENTLY IF EXISTS \"i\""]
        );
        assert_eq!(
            translate("ALTER TABLE \"t\" ADD CONSTRAINT \"c\" CHECK (\"a\" > 0)"),
            [
                "ALTER TABLE \"t\" ADD CONSTRAINT \"c\" CHECK (\"a\" > 0) NOT VALID",
                "ALTER TABLE \"t\" VALIDATE CONSTRAINT \"c\""
            ]
        );
        assert_eq!(
            translate("ALTER TABLE \"t\" ADD CONSTRAINT \"u\" UNIQUE NULLS NOT DISTINCT (\"a\", \"b\")"),
            [
                "CREATE UNIQUE INDEX CONCURRENTLY \"u\" ON \"t\" (\"a\", \"b\") NULLS NOT DISTINCT",
                "ALTER TABLE \"t\" ADD CONSTRAINT \"u\" UNIQUE USING INDEX \"u\""
            ]
        );
        assert_eq!(
            translate("ALTER TABLE \"t\" ADD CONSTRAINT \"d\" UNIQUE (\"a\") DEFERRABLE INITIALLY DEFERRED"),
            [
                "CREATE UNIQUE INDEX CONCURRENTLY \"d\" ON \"t\" (\"a\")",
                "ALTER TABLE \"t\" ADD CONSTRAINT \"d\" UNIQUE USING INDEX \"d\" DEFERRABLE INITIALLY DEFERRED"
            ]
        );
        assert_eq!(
            translate("ALTER TABLE \"t\" ADD CONSTRAINT \"p\" PRIMARY KEY (\"id\")"),
            [
                "CREATE UNIQUE INDEX CONCURRENTLY \"p\" ON \"t\" (\"id\")",
                "ALTER TABLE \"t\" ADD CONSTRAINT \"p\" PRIMARY KEY USING INDEX \"p\""
            ]
        );
    }
}
