use super::md5::hex_digest;

const MAX_NAME_LENGTH: usize = 63;
const INDEX_DIGEST_LENGTH: usize = 8;
const TABLE_DIGEST_LENGTH: usize = 4;
const HASH_PART_SHARE: usize = 3;
const INVALID_START_PREFIX: char = 'D';
const UNDERSCORE: char = '_';

pub fn quoted(name: &str) -> String {
    format!("\"{name}\"")
}

pub fn truncated_table(name: &str) -> String {
    if name.chars().count() <= MAX_NAME_LENGTH {
        return name.to_owned();
    }
    let digest = hex_digest(name.as_bytes());
    format!(
        "{}{}",
        prefix(name, MAX_NAME_LENGTH.saturating_sub(TABLE_DIGEST_LENGTH)),
        prefix(&digest, TABLE_DIGEST_LENGTH)
    )
}

pub fn index_name(table: &str, columns: &[&str], suffix: &str) -> String {
    let digest_input: String = std::iter::once(table).chain(columns.iter().copied()).collect();
    let mut hash_part = format!(
        "{}{suffix}",
        prefix(&hex_digest(digest_input.as_bytes()), INDEX_DIGEST_LENGTH)
    );
    let joined = columns.join("_");
    let full = format!("{table}_{joined}_{hash_part}");
    if full.chars().count() <= MAX_NAME_LENGTH {
        return full;
    }
    let hash_part_limit = MAX_NAME_LENGTH / HASH_PART_SHARE;
    if hash_part.chars().count() > hash_part_limit {
        hash_part = prefix(&hash_part, hash_part_limit);
    }
    let other_length = (MAX_NAME_LENGTH.saturating_sub(hash_part.chars().count()) / 2).saturating_sub(1);
    let name = format!(
        "{}_{}_{hash_part}",
        prefix(table, other_length),
        prefix(&joined, other_length)
    );
    if name.starts_with(|first: char| first == UNDERSCORE || first.is_ascii_digit()) {
        let kept = prefix(&name, name.chars().count().saturating_sub(1));
        return format!("{INVALID_START_PREFIX}{kept}");
    }
    name
}

fn prefix(text: &str, length: usize) -> String {
    text.chars().take(length).collect()
}
