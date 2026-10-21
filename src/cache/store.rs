use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rayon::prelude::*;

use super::key::CacheKey;
use super::shard::{Entry, Shard};

const SHARD_COUNT: usize = 256;
const SHARD_EXTENSION: &str = "bin";
const TEMPORARY_EXTENSION: &str = "tmp";
const LOCK_EXTENSION: &str = "lock";
const STAMP_REFRESH: Duration = Duration::from_hours(24);
const SIZE_PREFIX: usize = size_of::<u32>();
const MAX_ENTRY_BYTES: usize = 256 * 1024 * 1024;

#[derive(Debug)]
pub struct Store {
    directory: PathBuf,
    shard_budget: usize,
    shards: Vec<Mutex<Option<Shard>>>,
}

impl Store {
    pub fn new(directory: PathBuf, max_bytes: usize) -> Self {
        Self {
            directory,
            shard_budget: max_bytes / SHARD_COUNT,
            shards: (0..SHARD_COUNT).map(|_| Mutex::new(None)).collect(),
        }
    }

    pub fn get(&self, key: CacheKey) -> Option<Vec<u8>> {
        let now = now();
        let compressed = self.with_shard(key, |shard| {
            let entry = shard.entries.get_mut(&key)?;
            if !entry.is_intact() {
                shard.entries.remove(&key);
                shard.dirty = true;
                return None;
            }
            if now.saturating_sub(entry.stamp) > STAMP_REFRESH.as_secs() {
                entry.stamp = now;
                shard.dirty = true;
            }
            Some(entry.data.clone())
        })??;
        decompress(&compressed)
    }

    pub fn put(&self, key: CacheKey, bytes: &[u8]) {
        if bytes.len() > MAX_ENTRY_BYTES {
            return;
        }
        let entry = Entry::new(now(), compress(bytes));
        self.with_shard(key, |shard| {
            shard.entries.insert(key, entry);
            shard.dirty = true;
        });
    }

    pub fn persist(&self) {
        let dirty: Vec<usize> = self
            .shards
            .iter()
            .enumerate()
            .filter(|(_, slot)| {
                slot.lock()
                    .is_ok_and(|guard| guard.as_ref().is_some_and(|shard| shard.dirty))
            })
            .map(|(index, _)| index)
            .collect();
        if dirty.is_empty() || fs::create_dir_all(&self.directory).is_err() {
            return;
        }
        dirty.par_iter().for_each(|index| self.persist_shard(*index));
    }

    fn persist_shard(&self, index: usize) {
        let Some(slot) = self.shards.get(index) else {
            return;
        };
        let Ok(mut guard) = slot.lock() else {
            return;
        };
        let Some(shard) = guard.as_mut() else {
            return;
        };
        let held = lock_shard(&self.shard_path(index));
        shard.absorb(self.load(index));
        shard.evict_to(self.shard_budget);
        shard.dirty = false;
        write_atomically(&self.shard_path(index), &shard.serialize());
        drop(held);
    }

    fn with_shard<Outcome>(
        &self,
        key: CacheKey,
        use_shard: impl FnOnce(&mut Shard) -> Outcome,
    ) -> Option<Outcome> {
        let index = key.shard();
        let outcome = use_shard(
            self.shards
                .get(index)?
                .lock()
                .ok()?
                .get_or_insert_with(|| self.load(index)),
        );
        Some(outcome)
    }

    fn load(&self, index: usize) -> Shard {
        fs::read(self.shard_path(index)).map_or_else(|_| Shard::default(), |bytes| Shard::parse(&bytes))
    }

    fn shard_path(&self, index: usize) -> PathBuf {
        self.directory.join(format!("{index:02x}.{SHARD_EXTENSION}"))
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

fn lock_shard(shard: &Path) -> Option<File> {
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(shard.with_extension(LOCK_EXTENSION))
        .ok()?;
    lock.lock().ok()?;
    Some(lock)
}

fn write_atomically(path: &Path, bytes: &[u8]) {
    let temporary = path.with_extension(format!("{}.{TEMPORARY_EXTENSION}", process::id()));
    let written = fs::write(&temporary, bytes).and_then(|()| fs::rename(&temporary, path));
    if written.is_err() {
        let _removed = fs::remove_file(&temporary);
    }
}

fn compress(bytes: &[u8]) -> Vec<u8> {
    let length = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
    let mut compressed = length.to_le_bytes().to_vec();
    compressed.extend(lz4_flex::compress(bytes));
    compressed
}

fn decompress(compressed: &[u8]) -> Option<Vec<u8>> {
    let (prefix, body) = compressed.split_at_checked(SIZE_PREFIX)?;
    let length = usize::try_from(u32::from_le_bytes(prefix.try_into().ok()?)).ok()?;
    if length > MAX_ENTRY_BYTES {
        return None;
    }
    lz4_flex::decompress(body, length).ok()
}
