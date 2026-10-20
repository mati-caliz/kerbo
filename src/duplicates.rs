mod collector;
mod cpd;
mod java;
mod python;
mod python_letters;

use std::collections::HashMap;

use tree_sitter::Node;

use crate::profile::DuplicateStyle;
use crate::rules::{Diagnostic, FileIdentity, RuleId};

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0100_0000_01b3;
const SUPPRESSION_OFF: &str = "CPD-OFF";
const SUPPRESSION_ON: &str = "CPD-ON";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpdToken {
    pub image: u64,
    pub line: usize,
    pub end_line: usize,
}

#[derive(Debug)]
pub struct PooledFile {
    pub identity: FileIdentity,
    pub limit: usize,
    pub tokens: Vec<CpdToken>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Suppression {
    Unchanged,
    Off,
    On,
}

pub fn cpd_tokens(root: Node<'_>, source: &[u8], style: DuplicateStyle) -> Option<Vec<CpdToken>> {
    match style {
        DuplicateStyle::CpdJava => Some(java::tokens(root, source)),
        DuplicateStyle::CpdPython => python::tokens(source),
        DuplicateStyle::Unmeasured => None,
    }
}

pub fn find_duplicates(mut files: Vec<PooledFile>) -> Vec<Diagnostic> {
    files.sort_by(|left, right| left.identity.absolute.cmp(&right.identity.absolute));
    let Some(tile) = files.iter().map(|file| file.limit.saturating_add(1)).min() else {
        return Vec::new();
    };
    let pool = cpd::Pool::new(&mut files);
    let mut occurrences: HashMap<(usize, u64), usize> = HashMap::new();
    let mut diagnostics = Vec::new();
    for found in pool.matches(tile) {
        let marks: Vec<cpd::MarkSpan> = found
            .marks
            .iter()
            .filter_map(|&mark| pool.span(mark, found.tokens))
            .collect();
        let content = pool.content_hash(found.marks.first().copied(), found.tokens);
        for (position, mark) in marks.iter().enumerate() {
            let Some(file) = files.get(mark.file).filter(|file| found.tokens > file.limit) else {
                continue;
            };
            let occurrence = occurrences.entry((mark.file, content)).or_default();
            *occurrence = occurrence.saturating_add(1);
            let others = marks
                .iter()
                .enumerate()
                .filter(|&(other, _)| other != position)
                .filter_map(|(_, other)| {
                    files
                        .get(other.file)
                        .map(|other_file| format!("{}:{}", other_file.identity.display, other.line))
                })
                .collect::<Vec<_>>()
                .join(", ");
            diagnostics.push(Diagnostic {
                path: file.identity.display.clone(),
                line: mark.line,
                column: 1,
                end_line: mark.end_line,
                rule: RuleId::DuplicateCode,
                message: format!(
                    "Este bloque de {} tokens está repetido en {others} (máximo {})",
                    found.tokens, file.limit
                ),
                value: found.tokens,
                limit: file.limit,
                fingerprint: format!(
                    "duplicate-code:{}:{content:016x}#{occurrence}",
                    file.identity.relative
                ),
                relative_path: file.identity.relative.clone(),
                absolute_path: file.identity.absolute.clone(),
            });
        }
    }
    diagnostics
}

fn image_hash(text: &str) -> u64 {
    fnv(FNV_OFFSET, text.as_bytes().iter().map(|&byte| u64::from(byte)))
}

fn fnv(seed: u64, values: impl Iterator<Item = u64>) -> u64 {
    values.fold(seed, |hash, value| (hash ^ value).wrapping_mul(FNV_PRIME))
}

fn suppression<'text>(comments: impl DoubleEndedIterator<Item = &'text str>) -> Suppression {
    comments
        .rev()
        .find_map(|comment| {
            if comment.contains(SUPPRESSION_OFF) {
                Some(Suppression::Off)
            } else if comment.contains(SUPPRESSION_ON) {
                Some(Suppression::On)
            } else {
                None
            }
        })
        .unwrap_or(Suppression::Unchanged)
}

#[derive(Debug, Default)]
struct SuppressionState {
    discarding: bool,
}

impl SuppressionState {
    const fn apply(&mut self, suppression: Suppression) -> bool {
        match suppression {
            Suppression::Off => self.discarding = true,
            Suppression::On => self.discarding = false,
            Suppression::Unchanged => {}
        }
        self.discarding
    }
}

#[cfg(test)]
mod tests;
