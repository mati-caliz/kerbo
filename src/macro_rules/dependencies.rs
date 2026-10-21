use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use toml::{Table, Value};

const MANIFEST: &str = "Cargo.toml";
const LOCKFILE: &str = "Cargo.lock";
const DEPENDENCY_TABLES: &[&str] = &["dependencies", "dev-dependencies", "build-dependencies"];
const TARGET_TABLE: &str = "target";
const PACKAGE_KEY: &str = "package";
const FEATURES_TABLE: &str = "features";
const OPTIONAL_KEY: &str = "optional";
const PATH_KEY: &str = "path";
const NAME_KEY: &str = "name";
const VERSION_KEY: &str = "version";
const CARGO_HOME_VARIABLE: &str = "CARGO_HOME";
const HOME_VARIABLES: &[&str] = &["HOME", "USERPROFILE"];
const CARGO_DIRECTORY: &str = ".cargo";
const REGISTRY_SOURCES: &[&str] = &["registry", "src"];
const VERSION_SEPARATOR: char = '-';
const IDENTIFIER_SEPARATOR: char = '_';

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    pub name: String,
    pub root: PathBuf,
}

// Las dependencias que el código nombra (`dep::macro!`), cada una con la carpeta de su código: la de
// `path` si es local, y si no la de la caché de cargo, con la versión que fija el Cargo.lock o, sin
// lockfile, la más nueva que haya bajada.
pub fn dependencies(crate_root: &Path) -> Vec<Dependency> {
    let Some(manifest) = read_table(&crate_root.join(MANIFEST)) else {
        return Vec::new();
    };
    let locked = locked_versions(crate_root);
    let downloaded = downloaded_crates();
    let mut found: Vec<Dependency> = declared(&manifest)
        .into_iter()
        .filter_map(|(name, declaration)| {
            let package = declaration
                .get(PACKAGE_KEY)
                .and_then(Value::as_str)
                .unwrap_or(&name)
                .to_owned();
            let requirement = declaration
                .get(VERSION_KEY)
                .and_then(Value::as_str)
                .unwrap_or_default();
            let root = match declaration.get(PATH_KEY).and_then(Value::as_str) {
                Some(path) => crate_root.join(path),
                None => registry_root(
                    &Wanted {
                        package: &package,
                        requirement,
                    },
                    &locked,
                    &downloaded,
                )?,
            };
            root.join(MANIFEST).is_file().then(|| Dependency {
                name: name.replace(VERSION_SEPARATOR, &IDENTIFIER_SEPARATOR.to_string()),
                root,
            })
        })
        .collect();
    found.sort_by(|left, right| left.name.cmp(&right.name));
    found.dedup_by(|left, right| left.name == right.name);
    found
}

// Con `--all-features` vale toda feature declarada: las de `[features]` y las dependencias
// opcionales.
pub fn declared_features(crate_root: &Path) -> HashSet<String> {
    let Some(manifest) = read_table(&crate_root.join(MANIFEST)) else {
        return HashSet::new();
    };
    let named = manifest
        .get(FEATURES_TABLE)
        .and_then(Value::as_table)
        .into_iter()
        .flat_map(|features| features.keys().cloned());
    let optional = declared(&manifest)
        .into_iter()
        .filter(|(_, declaration)| declaration.get(OPTIONAL_KEY).and_then(Value::as_bool) == Some(true))
        .map(|(name, _)| name);
    named.chain(optional).collect()
}

fn read_table(path: &Path) -> Option<Table> {
    fs::read_to_string(path).ok()?.parse::<Table>().ok()
}

fn declared(manifest: &Table) -> Vec<(String, Table)> {
    let targets = manifest
        .get(TARGET_TABLE)
        .and_then(Value::as_table)
        .into_iter()
        .flat_map(|targets| targets.values().filter_map(Value::as_table));
    std::iter::once(manifest)
        .chain(targets)
        .flat_map(|table| {
            DEPENDENCY_TABLES
                .iter()
                .filter_map(|key| table.get(*key).and_then(Value::as_table))
        })
        .flat_map(|dependencies| {
            dependencies
                .iter()
                .map(|(name, value)| (name.clone(), declaration_of(value)))
        })
        .collect()
}

// `dep = "1.2"` es lo mismo que `dep = { version = "1.2" }`.
fn declaration_of(value: &Value) -> Table {
    value.as_table().cloned().unwrap_or_else(|| {
        value
            .as_str()
            .map(|requirement| Table::from_iter([(VERSION_KEY.to_owned(), Value::from(requirement))]))
            .unwrap_or_default()
    })
}

fn locked_versions(crate_root: &Path) -> Vec<(String, String)> {
    let Some(lock) = crate_root
        .ancestors()
        .map(|directory| directory.join(LOCKFILE))
        .find(|path| path.is_file())
        .and_then(|path| read_table(&path))
    else {
        return Vec::new();
    };
    lock.get(PACKAGE_KEY)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|package| {
            Some((
                package.get(NAME_KEY)?.as_str()?.to_owned(),
                package.get(VERSION_KEY)?.as_str()?.to_owned(),
            ))
        })
        .collect()
}

fn downloaded_crates() -> Vec<PathBuf> {
    let cargo_home = env::var_os(CARGO_HOME_VARIABLE).map(PathBuf::from).or_else(|| {
        HOME_VARIABLES
            .iter()
            .find_map(env::var_os)
            .map(|home| PathBuf::from(home).join(CARGO_DIRECTORY))
    });
    let Some(sources) =
        cargo_home.map(|home| REGISTRY_SOURCES.iter().fold(home, |path, part| path.join(part)))
    else {
        return Vec::new();
    };
    fs::read_dir(sources)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|registry| fs::read_dir(registry.path()).ok())
        .flat_map(|entries| entries.filter_map(Result::ok).map(|entry| entry.path()))
        .collect()
}

struct Wanted<'wanted> {
    package: &'wanted str,
    requirement: &'wanted str,
}

// Sin lockfile, como cargo con un requisito `^`: la más nueva con el mismo número principal (o el
// mismo menor si el principal es 0) que no sea anterior a lo pedido.
fn registry_root(
    wanted: &Wanted<'_>,
    locked: &[(String, String)],
    downloaded: &[PathBuf],
) -> Option<PathBuf> {
    let package = wanted.package;
    let floor = numeric_version(
        wanted
            .requirement
            .trim_start_matches(|character: char| !character.is_ascii_digit()),
    );
    let candidates: Vec<&PathBuf> = downloaded
        .iter()
        .filter(|path| {
            version_of(path, package).is_some_and(|version| compatible(&numeric_version(version), &floor))
        })
        .collect();
    let pinned = locked
        .iter()
        .filter(|(name, _)| name == package)
        .find_map(|(_, version)| {
            candidates
                .iter()
                .find(|path| version_of(path, package) == Some(version.as_str()))
        });
    pinned
        .or_else(|| {
            candidates
                .iter()
                .max_by_key(|path| numeric_version(version_of(path, package).unwrap_or_default()))
        })
        .map(|path| (*path).clone())
}

fn version_of<'path>(path: &'path Path, package: &str) -> Option<&'path str> {
    let folder = path.file_name()?.to_str()?;
    let version = folder.strip_prefix(package)?.strip_prefix(VERSION_SEPARATOR)?;
    version
        .starts_with(|first: char| first.is_ascii_digit())
        .then_some(version)
}

fn numeric_version(version: &str) -> Vec<u64> {
    version
        .split(|character: char| !character.is_ascii_digit())
        .map_while(|part| part.parse().ok())
        .collect()
}

fn compatible(version: &[u64], floor: &[u64]) -> bool {
    let fixed = floor
        .iter()
        .position(|part| *part != 0)
        .map_or(floor.len(), |first| first.saturating_add(1))
        .min(floor.len());
    version.get(..fixed) == floor.get(..fixed) && version >= floor
}

#[cfg(test)]
mod tests;
