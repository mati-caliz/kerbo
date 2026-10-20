use std::cmp::Reverse;
use std::collections::HashMap;
use std::mem;

use super::collector::{Match, MatchCollector};
use super::{FNV_OFFSET, PooledFile, fnv};

const END_OF_FILE: u32 = 0;
const HASH_BASE: i32 = 37;
const JAVA_LOAD_NUMERATOR: usize = 3;
const JAVA_LOAD_DENOMINATOR: usize = 4;
const JAVA_SPREAD_SHIFT: u32 = 16;

#[derive(Debug, Clone, Copy)]
struct Position {
    file: usize,
    line: usize,
    end_line: usize,
    image: u64,
}

#[derive(Debug)]
pub struct MarkSpan {
    pub file: usize,
    pub line: usize,
    pub end_line: usize,
}

#[derive(Debug)]
pub struct Pool {
    code: Vec<u32>,
    positions: Vec<Option<Position>>,
}

impl Pool {
    pub fn new(files: &mut [PooledFile]) -> Self {
        let mut identifiers: HashMap<u64, u32> = HashMap::new();
        let mut code = Vec::new();
        let mut positions = Vec::new();
        for (file, pooled) in files.iter_mut().enumerate() {
            for token in mem::take(&mut pooled.tokens) {
                let next = u32::try_from(identifiers.len()).map_or(u32::MAX, |count| count.saturating_add(1));
                code.push(*identifiers.entry(token.image).or_insert(next));
                positions.push(Some(Position {
                    file,
                    line: token.line,
                    end_line: token.end_line,
                    image: token.image,
                }));
            }
            code.push(END_OF_FILE);
            positions.push(None);
        }
        Self { code, positions }
    }

    pub fn matches(&self, tile: usize) -> Vec<Match> {
        let mut collector = MatchCollector::new(&self.code, tile);
        for group in self.mark_groups(tile) {
            collector.collect(&group);
        }
        collector.into_matches()
    }

    pub fn span(&self, mark: usize, tokens: usize) -> Option<MarkSpan> {
        let first = self.positions.get(mark).copied().flatten()?;
        let last = self
            .positions
            .get(mark.saturating_add(tokens).saturating_sub(1))
            .copied()
            .flatten()?;
        Some(MarkSpan {
            file: first.file,
            line: first.line,
            end_line: last.end_line,
        })
    }

    pub fn content_hash(&self, mark: Option<usize>, tokens: usize) -> u64 {
        let images = mark
            .into_iter()
            .flat_map(|first| first..first.saturating_add(tokens))
            .filter_map(|index| self.positions.get(index).copied().flatten())
            .map(|position| position.image);
        fnv(FNV_OFFSET, images)
    }

    fn mark_groups(&self, tile: usize) -> Vec<Vec<usize>> {
        let mut hashed = rolling_hashes(&self.code, tile);
        hashed.sort_unstable();
        let chunks = hashed.chunk_by(|left, right| left.0 == right.0);
        let distinct = chunks.clone().count();
        let capacity = java_hash_map_capacity(self.code.len(), distinct);
        let mut groups: Vec<(usize, Reverse<usize>, Vec<usize>)> = chunks
            .filter(|chunk| chunk.len() > 1)
            .filter_map(|chunk| {
                let &(hash, _) = chunk.first()?;
                let &(_, first_inserted) = chunk.last()?;
                let marks = chunk.iter().map(|&(_, index)| index).collect();
                Some((java_bucket(hash, capacity), Reverse(first_inserted), marks))
            })
            .collect();
        groups.sort_unstable_by_key(|&(bucket, first_inserted, _)| (bucket, first_inserted));
        groups.into_iter().map(|(_, _, marks)| marks).collect()
    }
}

// CPD recorre los grupos en el orden de un HashMap de Java (por balde y, dentro del balde, por orden
// de inserción), y ese orden decide qué coincidencia queda cuando dos se pisan.
fn java_hash_map_capacity(requested: usize, entries: usize) -> usize {
    let mut capacity = requested.max(1).next_power_of_two();
    while entries > capacity.saturating_mul(JAVA_LOAD_NUMERATOR) / JAVA_LOAD_DENOMINATOR {
        capacity = capacity.saturating_mul(2);
    }
    capacity
}

fn java_bucket(hash: i32, capacity: usize) -> usize {
    let bits = u32::from_ne_bytes(hash.to_ne_bytes());
    let spread = bits ^ (bits >> JAVA_SPREAD_SHIFT);
    usize::try_from(spread).unwrap_or_default() & capacity.saturating_sub(1)
}

// Calca MatchAlgorithm.hash() de CPD, con su aritmética de int de 32 bits: los grupos son los tokens
// con el mismo hash, colisiones incluidas, y las colisiones cambian qué pares se descartan después.
fn rolling_hashes(code: &[u32], tile: usize) -> Vec<(i32, usize)> {
    let last_power = (0..tile).fold(1_i32, |power, _| power.wrapping_mul(HASH_BASE));
    let identifier = |index: usize| {
        code.get(index)
            .map_or(0, |&value| i32::try_from(value).unwrap_or(i32::MAX))
    };
    let mut hashed = Vec::new();
    let mut last_hash = 0_i32;
    let mut index = code.len();
    while index > 0 {
        index = index.saturating_sub(1);
        if identifier(index) != 0 {
            let leaving = identifier(index.saturating_add(tile));
            last_hash = HASH_BASE
                .wrapping_mul(last_hash)
                .wrapping_add(identifier(index))
                .wrapping_sub(last_power.wrapping_mul(leaving));
            hashed.push((last_hash, index));
            continue;
        }
        last_hash = 0;
        let end = index.saturating_add(1).saturating_sub(tile);
        while index > end {
            let previous = identifier(index.saturating_sub(1));
            last_hash = HASH_BASE.wrapping_mul(last_hash).wrapping_add(previous);
            if previous == 0 {
                break;
            }
            index = index.saturating_sub(1);
        }
    }
    hashed
}
