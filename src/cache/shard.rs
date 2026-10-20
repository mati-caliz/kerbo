use std::cmp::Reverse;
use std::collections::HashMap;

use super::codec::{Corrupt, Reader, Writer};
use super::key::{CacheKey, checksum};

const MAGIC: &[u8] = b"KRBC";
const ENTRY_OVERHEAD: usize = 40;

#[derive(Debug)]
pub struct Entry {
    pub stamp: u64,
    pub checksum: u64,
    pub data: Vec<u8>,
}

impl Entry {
    pub fn new(stamp: u64, data: Vec<u8>) -> Self {
        Self {
            stamp,
            checksum: checksum(&data),
            data,
        }
    }

    pub fn is_intact(&self) -> bool {
        checksum(&self.data) == self.checksum
    }

    const fn footprint(&self) -> usize {
        self.data.len().saturating_add(ENTRY_OVERHEAD)
    }
}

#[derive(Debug, Default)]
pub struct Shard {
    pub entries: HashMap<CacheKey, Entry>,
    pub dirty: bool,
}

impl Shard {
    pub fn parse(bytes: &[u8]) -> Self {
        Self {
            entries: parse_entries(bytes).unwrap_or_default(),
            dirty: false,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut writer = Writer::default();
        for (key, entry) in &self.entries {
            let bits = key.bits();
            writer.fixed(u64::try_from(bits >> u64::BITS).unwrap_or_default());
            writer.fixed(u64::try_from(bits & u128::from(u64::MAX)).unwrap_or_default());
            writer.fixed(entry.stamp);
            writer.fixed(entry.checksum);
            writer.size(entry.data.len());
            writer.raw(&entry.data);
        }
        let mut bytes = MAGIC.to_vec();
        bytes.extend(writer.into_bytes());
        bytes
    }

    pub fn evict_to(&mut self, budget: usize) {
        let total: usize = self.entries.values().map(Entry::footprint).sum();
        if total <= budget {
            return;
        }
        let mut by_age: Vec<(u64, CacheKey, usize)> = self
            .entries
            .iter()
            .map(|(&key, entry)| (entry.stamp, key, entry.footprint()))
            .collect();
        by_age.sort_unstable_by_key(|&(stamp, _, _)| Reverse(stamp));
        let mut kept: usize = 0;
        for (_, key, footprint) in by_age {
            kept = kept.saturating_add(footprint);
            if kept > budget {
                self.entries.remove(&key);
            }
        }
        self.dirty = true;
    }
}

fn parse_entries(bytes: &[u8]) -> Result<HashMap<CacheKey, Entry>, Corrupt> {
    let body = bytes.strip_prefix(MAGIC).ok_or(Corrupt)?;
    let mut reader = Reader::new(body);
    let mut entries = HashMap::new();
    while !reader.is_empty() {
        let high = u128::from(reader.fixed()?);
        let low = u128::from(reader.fixed()?);
        let stamp = reader.fixed()?;
        let checksum = reader.fixed()?;
        let length = reader.size()?;
        let data = reader.take(length)?.to_vec();
        entries.insert(
            CacheKey::from_bits((high << u64::BITS) | low),
            Entry {
                stamp,
                checksum,
                data,
            },
        );
    }
    Ok(entries)
}
