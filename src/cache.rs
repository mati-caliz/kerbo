mod analysis_codec;
mod codec;
mod key;
mod lint_codec;
mod location;
mod shard;
mod store;

use std::env;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

pub use key::CacheKey;
use key::{EntryKind, KeyBuilder};
use store::Store;

use crate::analysis::FileAnalysis;
use crate::language::Language;
use crate::limits::Limits;
use crate::rules::RuleId;

pub const DEFAULT_MAX_BYTES: usize = 256 * 1024 * 1024;
const FORMAT_DIRECTORY: &str = "analysis-v2";
const FORMAT_VERSION: u64 = 2;

#[derive(Debug, Clone, Copy)]
pub struct AnalysisSubject<'subject> {
    pub language: Language,
    pub source: &'subject [u8],
    pub context: Option<u64>,
}

#[derive(Debug)]
pub struct AnalysisCache {
    store: Store,
    salt: u64,
}

impl AnalysisCache {
    pub fn open_default() -> Option<Self> {
        location::default_directory()
            .map(|directory| Self::open(directory.join(FORMAT_DIRECTORY), DEFAULT_MAX_BYTES))
    }

    pub fn open(directory: PathBuf, max_bytes: usize) -> Self {
        Self {
            store: Store::new(directory, max_bytes),
            salt: build_salt(),
        }
    }

    pub fn analysis_key(&self, subject: AnalysisSubject<'_>, limits: Limits) -> CacheKey {
        let AnalysisSubject {
            language,
            source,
            context,
        } = subject;
        let mut builder = KeyBuilder::new(self.salt, EntryKind::FileAnalysis);
        language.hash(&mut builder);
        context.hash(&mut builder);
        for rule in RuleId::ALL {
            limits.get(rule).hash(&mut builder);
        }
        builder.content(source);
        builder.key()
    }

    pub fn analysis(&self, key: CacheKey) -> Option<FileAnalysis> {
        self.store
            .get(key)
            .and_then(|bytes| analysis_codec::decode(&bytes).ok())
    }

    pub fn remember(&self, key: CacheKey, analysis: &FileAnalysis) {
        self.store.put(key, &analysis_codec::encode(analysis));
    }

    pub fn persist(&self) {
        self.store.persist();
    }
}

fn build_salt() -> u64 {
    let mut hasher = KeyBuilder::new(FORMAT_VERSION, EntryKind::FileAnalysis);
    env!("CARGO_PKG_VERSION").hash(&mut hasher);
    let binary = env::current_exe().and_then(fs::metadata);
    if let Ok(metadata) = binary {
        metadata.len().hash(&mut hasher);
        metadata
            .modified()
            .ok()
            .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
            .hash(&mut hasher);
    }
    hasher.finish()
}

#[cfg(test)]
mod tests;
