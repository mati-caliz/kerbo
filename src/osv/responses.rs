use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::record::Record;

pub const QUERY_TTL: Duration = Duration::from_hours(24);
const QUERIES_FILE: &str = "queries.json";
const RECORDS_DIRECTORY: &str = "vulns";
const RECORD_EXTENSION: &str = "json";
const TEMPORARY_EXTENSION: &str = "tmp";
const UNSAFE_FILE_CHARACTERS: &[char] = &['/', '\\', ':', '.'];
const SAFE_REPLACEMENT: &str = "_";
const UTC_SUFFIX: char = 'Z';
const FRACTION_SEPARATOR: char = '.';
const COMPARED_FRACTION_DIGITS: usize = 6;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Listed {
    pub id: String,
    #[serde(default)]
    pub modified: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedQuery {
    fetched: u64,
    vulns: Vec<Listed>,
}

#[derive(Debug)]
pub struct Responses {
    directory: PathBuf,
    now: u64,
    queries: BTreeMap<String, CachedQuery>,
    dirty: bool,
}

impl Responses {
    pub fn open(directory: PathBuf, now: u64) -> Self {
        let queries = fs::read(directory.join(QUERIES_FILE))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self {
            directory,
            now,
            queries,
            dirty: false,
        }
    }

    pub fn listed(&self, key: &str) -> Option<Vec<Listed>> {
        self.queries
            .get(key)
            .filter(|cached| self.now.saturating_sub(cached.fetched) < QUERY_TTL.as_secs())
            .map(|cached| cached.vulns.clone())
    }

    pub fn remember_listed(&mut self, key: String, vulns: Vec<Listed>) {
        self.queries.insert(
            key,
            CachedQuery {
                fetched: self.now,
                vulns,
            },
        );
        self.dirty = true;
    }

    pub fn record(&self, listed: &Listed) -> Option<Record> {
        let bytes = fs::read(self.record_path(&listed.id)).ok()?;
        let record: Record = serde_json::from_slice(&bytes).ok()?;
        same_instant(&record.modified, &listed.modified).then_some(record)
    }

    pub fn remember_record(&self, id: &str, bytes: &[u8]) {
        let path = self.record_path(id);
        if let Some(parent) = path.parent()
            && fs::create_dir_all(parent).is_ok()
        {
            write_atomically(&path, bytes);
        }
    }

    pub fn persist(&self) {
        if !self.dirty || fs::create_dir_all(&self.directory).is_err() {
            return;
        }
        if let Ok(bytes) = serde_json::to_vec(&self.queries) {
            write_atomically(&self.directory.join(QUERIES_FILE), &bytes);
        }
    }

    fn record_path(&self, id: &str) -> PathBuf {
        let safe = id.replace(UNSAFE_FILE_CHARACTERS, SAFE_REPLACEMENT);
        self.directory
            .join(RECORDS_DIRECTORY)
            .join(format!("{safe}.{RECORD_EXTENSION}"))
    }
}

pub fn same_instant(left: &str, right: &str) -> bool {
    let truncated = |stamp: &str| {
        let stamp = stamp.trim_end_matches(UTC_SUFFIX);
        let (whole, fraction) = stamp.split_once(FRACTION_SEPARATOR).unwrap_or((stamp, ""));
        let digits: String = fraction
            .chars()
            .chain(std::iter::repeat('0'))
            .take(COMPARED_FRACTION_DIGITS)
            .collect();
        format!("{whole}{FRACTION_SEPARATOR}{digits}")
    };
    truncated(left) == truncated(right)
}

fn write_atomically(path: &Path, bytes: &[u8]) {
    let temporary = path.with_extension(format!("{}.{TEMPORARY_EXTENSION}", process::id()));
    let written = fs::write(&temporary, bytes).and_then(|()| fs::rename(&temporary, path));
    if written.is_err() {
        let _removed = fs::remove_file(&temporary);
    }
}
