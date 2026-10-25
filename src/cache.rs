mod analysis_codec;
mod budget;
mod chain_codec;
mod codec;
mod go_package_codec;
mod key;
mod lint_codec;
pub mod location;
mod secret_codec;
mod shard;
mod store;
mod summary_codec;
mod taint_codec;

use std::collections::HashMap;
use std::env;
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::UNIX_EPOCH;

pub use key::CacheKey;
use key::{EntryKind, KeyBuilder};
use store::Store;

use crate::analysis::FileAnalysis;
use crate::language::Language;
use crate::limits::Limits;
use crate::rules::RuleId;
use crate::secrets::{PathProfile, SecretFinding};

const FORMAT_DIRECTORY: &str = "analysis-v2";
const FORMAT_VERSION: u64 = 30;

#[derive(Debug, Clone, Copy)]
pub struct AnalysisSubject<'subject> {
    pub language: Language,
    pub source: &'subject [u8],
    pub context: Option<u64>,
}

pub fn cache_root() -> Option<PathBuf> {
    location::default_directory()
}

pub fn runtime_identity() -> u64 {
    let mut hasher = DefaultHasher::new();
    build_salt().hash(&mut hasher);
    budget::max_bytes(None).hash(&mut hasher);
    hasher.finish()
}

type OpenCaches = Mutex<HashMap<(PathBuf, usize), Arc<AnalysisCache>>>;

static OPEN_CACHES: OnceLock<OpenCaches> = OnceLock::new();

#[derive(Debug)]
pub struct AnalysisCache {
    store: Store,
    salt: u64,
}

impl AnalysisCache {
    pub fn shared(configured_megabytes: Option<usize>) -> Option<Arc<Self>> {
        let max_bytes = budget::max_bytes(configured_megabytes);
        let directory = location::default_directory()?.join(FORMAT_DIRECTORY);
        let cache = Arc::clone(
            OPEN_CACHES
                .get_or_init(OpenCaches::default)
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .entry((directory.clone(), max_bytes))
                .or_insert_with(|| Arc::new(Self::open(directory, max_bytes))),
        );
        Some(cache)
    }

    pub fn open(directory: PathBuf, max_bytes: usize) -> Self {
        Self {
            store: Store::new(directory, max_bytes),
            salt: build_salt(),
        }
    }

    pub fn analysis_key(&self, subject: AnalysisSubject<'_>, limits: &Limits) -> CacheKey {
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

    pub fn secrets_key(&self, profile: &PathProfile, source: &[u8]) -> CacheKey {
        let mut builder = KeyBuilder::new(self.salt, EntryKind::Secrets);
        profile.signature().hash(&mut builder);
        builder.content(source);
        builder.key()
    }

    pub fn secrets(&self, key: CacheKey) -> Option<Vec<SecretFinding>> {
        self.store
            .get(key)
            .and_then(|bytes| secret_codec::decode(&bytes).ok())
    }

    pub fn remember_secrets(&self, key: CacheKey, findings: &[SecretFinding]) {
        self.store.put(key, &secret_codec::encode(findings));
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
mod chain_tests;
#[cfg(test)]
mod tests;
