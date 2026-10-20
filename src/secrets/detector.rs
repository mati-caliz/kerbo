use std::collections::HashMap;
use std::ops::Range;

use regex::bytes::Regex;

use super::allowlist::Evidence;
use super::catalog::{Catalog, PathProfile, SecretRule};
use super::decode::{Decoder, current_line};
use super::location::LineIndex;

const NEWLINE: u8 = b'\n';
const ALLOW_SIGNATURE: &[u8] = b"gitleaks:allow";
const FIRST_LINE: usize = 1;
const FIRST_COLUMN: usize = 1;
const MAX_DECODE_DEPTH: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub rule: usize,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub secret: Vec<u8>,
}

struct Active<'rule> {
    index: usize,
    rule: &'rule SecretRule,
    pattern: &'rule Regex,
}

struct Pass<'pass> {
    catalog: &'pass Catalog,
    content: &'pass [u8],
    text: &'pass [u8],
    profile: &'pass PathProfile,
    lines: &'pass LineIndex,
    decoder: &'pass Decoder,
}

pub fn detect(catalog: &Catalog, content: &[u8], profile: &PathProfile) -> Vec<Candidate> {
    let lines = LineIndex::new(content);
    let mut decoder = Decoder::default();
    let mut candidates = Vec::new();
    let mut text = content.to_vec();
    for depth in 0..=MAX_DECODE_DEPTH {
        let pass = Pass {
            catalog,
            content,
            text: &text,
            profile,
            lines: &lines,
            decoder: &decoder,
        };
        candidates.extend(pass.run());
        if depth == MAX_DECODE_DEPTH {
            break;
        }
        match decoder.decode(&text) {
            Some(next_text) => text = next_text,
            None => break,
        }
    }
    candidates
}

impl Pass<'_> {
    fn run(&self) -> Vec<Candidate> {
        let lowered = self.text.to_ascii_lowercase();
        self.catalog
            .triggered_rules(&lowered)
            .into_iter()
            .flat_map(|index| {
                self.catalog
                    .rules()
                    .get(index)
                    .map(|rule| self.rule_candidates(index, rule))
                    .unwrap_or_default()
            })
            .collect()
    }

    fn rule_candidates(&self, index: usize, rule: &SecretRule) -> Vec<Candidate> {
        if rule
            .allowlists
            .iter()
            .any(|allowlist| allowlist.skips_file(self.profile))
        {
            return Vec::new();
        }
        let path_matches = rule.path.is_none_or(|path| self.profile.matches(path));
        if !path_matches {
            return Vec::new();
        }
        if !rule.has_pattern() {
            return if self.decoder.latest().is_none() {
                path_only(index, rule)
            } else {
                Vec::new()
            };
        }
        let Some(pattern) = rule.pattern() else {
            return Vec::new();
        };
        pattern
            .find_iter(self.text)
            .filter_map(|found| self.candidate(&Active { index, rule, pattern }, found.range()))
            .collect()
    }

    fn placement(&self, span: &Range<usize>, matched: &[u8]) -> Option<(Range<usize>, Option<Range<usize>>)> {
        if self.decoder.latest().is_none() {
            return Some((span.start..span.start.saturating_add(matched.len()), None));
        }
        let overlapping = self.decoder.overlapping(span);
        if overlapping.is_empty() {
            return None;
        }
        let original = self.decoder.original(&overlapping, span);
        Some((original, Some(current_line(&overlapping, self.text))))
    }

    fn candidate(&self, active: &Active<'_>, span: Range<usize>) -> Option<Candidate> {
        let matched = trim_newlines(self.text.get(span.clone())?);
        let (original, decoded_line) = self.placement(&span, matched)?;
        let located = self.lines.locate(self.content, original);
        let line = self.content.get(located.text.clone()).unwrap_or_default();
        if contains(line, ALLOW_SIGNATURE) {
            return None;
        }
        let current = decoded_line
            .and_then(|range| self.text.get(range))
            .unwrap_or(line);
        let secret = secret_of(active.pattern, matched, active.rule.secret_group)?;
        if active
            .rule
            .entropy
            .is_some_and(|threshold| shannon_entropy(secret) <= threshold)
        {
            return None;
        }
        let evidence = Evidence {
            secret,
            matched,
            line: current,
        };
        let allowed = std::iter::once(self.catalog.global())
            .chain(&active.rule.allowlists)
            .any(|allowlist| allowlist.skips_finding(&evidence, self.profile));
        (!allowed).then(|| Candidate {
            rule: active.index,
            line: located.line,
            column: located.column,
            end_line: located.end_line,
            secret: secret.to_vec(),
        })
    }
}

fn path_only(index: usize, rule: &SecretRule) -> Vec<Candidate> {
    if rule.path.is_none() {
        return Vec::new();
    }
    vec![Candidate {
        rule: index,
        line: FIRST_LINE,
        column: FIRST_COLUMN,
        end_line: FIRST_LINE,
        secret: Vec::new(),
    }]
}

fn trim_newlines(text: &[u8]) -> &[u8] {
    let start = text
        .iter()
        .position(|&byte| byte != NEWLINE)
        .unwrap_or(text.len());
    let end = text
        .iter()
        .rposition(|&byte| byte != NEWLINE)
        .map_or(start, |last| last.saturating_add(1));
    text.get(start..end).unwrap_or_default()
}

fn secret_of<'text>(pattern: &Regex, matched: &'text [u8], group: Option<usize>) -> Option<&'text [u8]> {
    let Some(captures) = pattern.captures(matched) else {
        return Some(matched);
    };
    if captures.len() < 2 {
        return Some(matched);
    }
    if let Some(group) = group.filter(|&group| group > 0) {
        if group >= captures.len() {
            return None;
        }
        return Some(captures.get(group).map_or(&[][..], |found| found.as_bytes()));
    }
    let first_filled = captures
        .iter()
        .skip(1)
        .flatten()
        .find(|found| !found.as_bytes().is_empty());
    Some(first_filled.map_or(matched, |found| found.as_bytes()))
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|window| window == needle)
}

pub fn shannon_entropy(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let text = String::from_utf8_lossy(data);
    let mut counts: HashMap<char, u32> = HashMap::new();
    for character in text.chars() {
        let count = counts.entry(character).or_default();
        *count = count.saturating_add(1);
    }
    let length = f64::from(u32::try_from(data.len()).unwrap_or(u32::MAX));
    counts
        .values()
        .map(|&count| {
            let frequency = f64::from(count) / length;
            -frequency * frequency.log2()
        })
        .sum()
}
