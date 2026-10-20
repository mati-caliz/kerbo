use std::hash::{DefaultHasher, Hasher};

const LOW_SEED: u64 = 0x6b65_7262_6f2d_6c6f;
const HIGH_SEED: u64 = 0x6b65_7262_6f2d_6869;
const SHARD_SHIFT: u32 = u128::BITS - u8::BITS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CacheKey(u128);

impl CacheKey {
    pub const fn from_bits(bits: u128) -> Self {
        Self(bits)
    }

    pub const fn bits(self) -> u128 {
        self.0
    }

    pub fn shard(self) -> usize {
        usize::from(u8::try_from(self.0 >> SHARD_SHIFT).unwrap_or_default())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    FileAnalysis,
    Secrets,
}

impl EntryKind {
    const fn tag(self) -> u64 {
        match self {
            Self::FileAnalysis => 1,
            Self::Secrets => 2,
        }
    }
}

#[derive(Debug, Clone)]
pub struct KeyBuilder {
    low: DefaultHasher,
    high: DefaultHasher,
}

impl KeyBuilder {
    pub fn new(salt: u64, kind: EntryKind) -> Self {
        let mut builder = Self {
            low: DefaultHasher::new(),
            high: DefaultHasher::new(),
        };
        builder.low.write_u64(LOW_SEED);
        builder.high.write_u64(HIGH_SEED);
        builder.write_u64(salt);
        builder.write_u64(kind.tag());
        builder
    }

    pub fn content(&mut self, bytes: &[u8]) {
        self.write_usize(bytes.len());
        self.write(bytes);
    }

    pub fn key(&self) -> CacheKey {
        CacheKey((u128::from(self.high.finish()) << u64::BITS) | u128::from(self.low.finish()))
    }
}

impl Hasher for KeyBuilder {
    fn write(&mut self, bytes: &[u8]) {
        self.low.write(bytes);
        self.high.write(bytes);
    }

    fn finish(&self) -> u64 {
        self.low.finish()
    }
}

pub fn checksum(bytes: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    hasher.write(bytes);
    hasher.finish()
}
