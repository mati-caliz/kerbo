mod allowlist;
mod catalog;
mod decode;
mod detector;
mod dialect;
mod location;
mod sniff;

use catalog::Catalog;
pub use catalog::PathProfile;
use detector::Candidate;

const GENERIC_MARKER: &str = "generic";
const PREVIEW_CHARACTERS: usize = 4;
const SHORTEST_PREVIEWED_SECRET: usize = 12;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretFinding {
    pub kind: String,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub preview: String,
    pub length: usize,
}

pub fn path_profile(relative: &str) -> Option<PathProfile> {
    catalog::shared()?.profile(relative)
}

pub fn find_secrets(content: &[u8], profile: &PathProfile) -> Vec<SecretFinding> {
    let Some(catalog) = catalog::shared() else {
        return Vec::new();
    };
    if content.is_empty() || sniff::is_binary(content) {
        return Vec::new();
    }
    let candidates = detector::detect(catalog, content, profile);
    candidates
        .iter()
        .filter(|candidate| !shadowed_by_specific_rule(catalog, candidate, &candidates))
        .map(|candidate| finding(catalog, candidate))
        .collect()
}

fn shadowed_by_specific_rule(catalog: &Catalog, candidate: &Candidate, all: &[Candidate]) -> bool {
    is_generic(catalog, candidate.rule)
        && all.iter().any(|other| {
            other.line == candidate.line
                && other.rule != candidate.rule
                && !is_generic(catalog, other.rule)
                && contains(&other.secret, &candidate.secret)
        })
}

fn is_generic(catalog: &Catalog, rule: usize) -> bool {
    catalog
        .rule_id(rule)
        .is_some_and(|id| id.to_ascii_lowercase().contains(GENERIC_MARKER))
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    needle.is_empty() || haystack.windows(needle.len()).any(|window| window == needle)
}

fn finding(catalog: &Catalog, candidate: &Candidate) -> SecretFinding {
    let secret = String::from_utf8_lossy(&candidate.secret);
    let length = secret.chars().count();
    let preview = if length >= SHORTEST_PREVIEWED_SECRET {
        secret.chars().take(PREVIEW_CHARACTERS).collect()
    } else {
        String::new()
    };
    SecretFinding {
        kind: catalog.rule_id(candidate.rule).unwrap_or_default().to_owned(),
        line: candidate.line,
        column: candidate.column,
        end_line: candidate.end_line,
        preview,
        length,
    }
}

#[cfg(test)]
mod tests;
