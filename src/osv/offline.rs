use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::lockfiles::Dependency;
use super::online::query_key;
use super::record::Record;
use super::transport::Transport;
use super::zip::{Archive, Entry};
use super::{Ecosystem, normalized_name};
use crate::i18n::OsvProblem;

const DATABASE_URL: &str = "https://osv-vulnerabilities.storage.googleapis.com";
const ARCHIVE_NAME: &str = "all.zip";
const PARTIAL_ARCHIVE_NAME: &str = "all.zip.part";
const INDEX_NAME: &str = "index.tsv";
const PARTIAL_INDEX_NAME: &str = "index.tsv.part";
const FIELD_SEPARATOR: char = '\t';

#[derive(Debug, Deserialize)]
struct Listing {
    #[serde(default)]
    affected: Vec<ListedPackage>,
}

#[derive(Debug, Deserialize)]
struct ListedPackage {
    package: Option<ListedName>,
}

#[derive(Debug, Deserialize)]
struct ListedName {
    name: String,
    ecosystem: String,
}

pub fn download(
    ecosystem: Ecosystem,
    transport: &dyn Transport,
    database: &Path,
) -> Result<(usize, PathBuf), OsvProblem> {
    let directory = database.join(ecosystem.as_str());
    fs::create_dir_all(&directory).map_err(|error| OsvProblem::Unavailable(error.to_string()))?;
    let partial = directory.join(PARTIAL_ARCHIVE_NAME);
    let archive_path = directory.join(ARCHIVE_NAME);
    let url = format!("{DATABASE_URL}/{}/{ARCHIVE_NAME}", ecosystem.as_str());
    transport
        .download(&url, &partial)
        .and_then(|()| fs::rename(&partial, &archive_path).map_err(|error| error.to_string()))
        .map_err(OsvProblem::Unavailable)?;
    let count = build_index(ecosystem, &directory).map_err(OsvProblem::InvalidResponse)?;
    Ok((count, archive_path))
}

fn build_index(ecosystem: Ecosystem, directory: &Path) -> Result<usize, String> {
    let file = File::open(directory.join(ARCHIVE_NAME)).map_err(|error| error.to_string())?;
    let mut archive = Archive::open(file);
    let entries = archive.entries()?;
    let mut index = String::new();
    for entry in &entries {
        let bytes = archive.read(*entry)?;
        let Ok(listing) = serde_json::from_slice::<Listing>(&bytes) else {
            continue;
        };
        let mut names: Vec<String> = listing
            .affected
            .into_iter()
            .filter_map(|affected| affected.package)
            .filter(|package| package.ecosystem == ecosystem.as_str())
            .map(|package| normalized_name(ecosystem, &package.name))
            .collect();
        names.sort();
        names.dedup();
        for name in names {
            let _written = writeln!(
                index,
                "{name}{FIELD_SEPARATOR}{}{FIELD_SEPARATOR}{}{FIELD_SEPARATOR}{}",
                entry.offset, entry.size, entry.method
            );
        }
    }
    let partial = directory.join(PARTIAL_INDEX_NAME);
    fs::write(&partial, index)
        .and_then(|()| fs::rename(&partial, directory.join(INDEX_NAME)))
        .map_err(|error| error.to_string())?;
    Ok(entries.len())
}

fn read_index(path: &Path, wanted: &HashSet<String>) -> Result<HashMap<String, Vec<Entry>>, OsvProblem> {
    let text =
        fs::read_to_string(path).map_err(|_| OsvProblem::DatabaseMissing(path.display().to_string()))?;
    let mut index: HashMap<String, Vec<Entry>> = HashMap::new();
    for line in text.lines() {
        let mut fields = line.split(FIELD_SEPARATOR);
        let (Some(name), Some(offset), Some(size), Some(method)) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        if !wanted.contains(name) {
            continue;
        }
        let (Ok(offset), Ok(size), Ok(method)) = (offset.parse(), size.parse(), method.parse()) else {
            continue;
        };
        index
            .entry(name.to_owned())
            .or_default()
            .push(Entry { offset, size, method });
    }
    Ok(index)
}

pub fn lookup(
    dependencies: &[&Dependency],
    database: &Path,
) -> Result<BTreeMap<String, Vec<Record>>, OsvProblem> {
    let mut by_ecosystem: BTreeMap<&'static str, (Ecosystem, Vec<&Dependency>)> = BTreeMap::new();
    for dependency in dependencies {
        by_ecosystem
            .entry(dependency.ecosystem.as_str())
            .or_insert_with(|| (dependency.ecosystem, Vec::new()))
            .1
            .push(dependency);
    }
    let mut found = BTreeMap::new();
    for (ecosystem, group) in by_ecosystem.into_values() {
        found.extend(lookup_ecosystem(
            ecosystem,
            &group,
            &database.join(ecosystem.as_str()),
        )?);
    }
    Ok(found)
}

fn lookup_ecosystem(
    ecosystem: Ecosystem,
    dependencies: &[&Dependency],
    directory: &Path,
) -> Result<BTreeMap<String, Vec<Record>>, OsvProblem> {
    let wanted: HashSet<String> = dependencies
        .iter()
        .map(|dependency| normalized_name(ecosystem, &dependency.name))
        .collect();
    let index = read_index(&directory.join(INDEX_NAME), &wanted)?;
    let archive_path = directory.join(ARCHIVE_NAME);
    let file = File::open(&archive_path)
        .map_err(|_| OsvProblem::DatabaseMissing(archive_path.display().to_string()))?;
    let mut archive = Archive::open(file);
    let mut parsed: HashMap<u64, Option<Record>> = HashMap::new();
    let mut found = BTreeMap::new();
    for dependency in dependencies {
        let entries = index
            .get(&normalized_name(ecosystem, &dependency.name))
            .cloned()
            .unwrap_or_default();
        let mut records = Vec::new();
        for entry in entries {
            let record = parsed.entry(entry.offset).or_insert_with(|| {
                archive
                    .read(entry)
                    .ok()
                    .and_then(|bytes| serde_json::from_slice::<Record>(&bytes).ok())
            });
            if let Some(record) = record.as_ref().filter(|record| record.affects(dependency)) {
                records.push(record.clone());
            }
        }
        found.insert(query_key(dependency), records);
    }
    Ok(found)
}
