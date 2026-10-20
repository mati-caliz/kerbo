use std::fs;
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
const STAMP_REFRESH: Duration = Duration::from_hours(24);
const SIZE_PREFIX: usize = size_of::<u32>();
const MAX_ENTRY_BYTES: usize = 256 * 1024 * 1024;

#[derive(Debug)]
pub struct Store {
    directory: PathBuf,
    shard_budget: usize,
    now: u64,
    shards: Vec<Mutex<Option<Shard>>>,
}

impl Store {
    pub fn new(directory: PathBuf, max_bytes: usize) -> Self {
        Self {
            directory,
            shard_budget: max_bytes / SHARD_COUNT,
            now: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_secs()),
            shards: (0..SHARD_COUNT).map(|_| Mutex::new(None)).collect(),
        }
    }

    pub fn get(&self, key: CacheKey) -> Option<Vec<u8>> {
        let compressed = self.with_shard(key, |shard| {
            let entry = shard.entries.get_mut(&key)?;
            if !entry.is_intact() {
                shard.entries.remove(&key);
                shard.dirty = true;
                return None;
            }
            if self.now.saturating_sub(entry.stamp) > STAMP_REFRESH.as_secs() {
                entry.stamp = self.now;
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
        let entry = Entry::new(self.now, compress(bytes));
        self.with_shard(key, |shard| {
            shard.entries.insert(key, entry);
            shard.dirty = true;
        });
    }

    pub fn persist(&self) {
        let pending: Vec<(usize, Vec<u8>)> = self
            .shards
            .par_iter()
            .enumerate()
            .filter_map(|(index, slot)| {
                let bytes = slot
                    .lock()
                    .ok()?
                    .as_mut()
                    .filter(|shard| shard.dirty)
                    .map(|shard| {
                        shard.absorb(self.load(index));
                        shard.evict_to(self.shard_budget);
                        shard.dirty = false;
                        shard.serialize()
                    })?;
                Some((index, bytes))
            })
            .collect();
        if pending.is_empty() || fs::create_dir_all(&self.directory).is_err() {
            return;
        }
        pending
            .par_iter()
            .for_each(|(index, bytes)| write_atomically(&self.shard_path(*index), bytes));
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
