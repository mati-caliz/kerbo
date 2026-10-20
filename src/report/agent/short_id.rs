const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
const AVALANCHE_SHIFT: u32 = 33;
const AVALANCHE_FIRST: u64 = 0xff51_afd7_ed55_8ccd;
const AVALANCHE_SECOND: u64 = 0xc4ce_b9fe_1a85_ec53;
const MIN_LENGTH: usize = 4;

pub(super) fn shortest_unique(fingerprints: &[&str]) -> Vec<String> {
    let hashes: Vec<String> = fingerprints.iter().map(|fingerprint| full(fingerprint)).collect();
    let mut sorted: Vec<&str> = hashes.iter().map(String::as_str).collect();
    sorted.sort_unstable();
    hashes
        .iter()
        .map(|hash| {
            let length = unique_length(hash, &sorted);
            hash.get(..length).unwrap_or(hash).to_owned()
        })
        .collect()
}

fn full(fingerprint: &str) -> String {
    let hash = fingerprint.bytes().fold(FNV_OFFSET_BASIS, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(FNV_PRIME)
    });
    format!("{:016x}", avalanche(hash))
}

const fn avalanche(hash: u64) -> u64 {
    let mixed = (hash ^ (hash >> AVALANCHE_SHIFT)).wrapping_mul(AVALANCHE_FIRST);
    let mixed = (mixed ^ (mixed >> AVALANCHE_SHIFT)).wrapping_mul(AVALANCHE_SECOND);
    mixed ^ (mixed >> AVALANCHE_SHIFT)
}

fn unique_length(hash: &str, sorted: &[&str]) -> usize {
    let position = sorted.partition_point(|other| *other < hash);
    let neighbors = [
        position.checked_sub(1).and_then(|index| sorted.get(index)),
        sorted.get(position + 1),
    ];
    let shared = neighbors
        .into_iter()
        .flatten()
        .map(|neighbor| common_prefix(hash, neighbor))
        .max()
        .unwrap_or_default();
    (shared + 1).clamp(MIN_LENGTH, hash.len())
}

fn common_prefix(left: &str, right: &str) -> usize {
    left.bytes()
        .zip(right.bytes())
        .take_while(|(left_byte, right_byte)| left_byte == right_byte)
        .count()
}
