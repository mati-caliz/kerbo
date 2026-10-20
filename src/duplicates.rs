mod collector;
mod cpd;
mod dupl;
mod go_kinds;
mod go_statements;
mod go_syntax;
mod go_types;
mod java;
mod python;
mod python_letters;
mod sonar;
mod suffix_tree;

use std::collections::HashMap;
use std::path::Path;

use tree_sitter::Node;

pub use go_syntax::SyntaxNode;
pub use sonar::RepeatedBlock;

use sonar::RepeatedKind;

use crate::profile::DuplicateStyle;
use crate::rules::{Diagnostic, FileIdentity, RuleId};

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0100_0000_01b3;
const SUPPRESSION_OFF: &str = "CPD-OFF";
const SUPPRESSION_ON: &str = "CPD-ON";
const TOKEN_UNIT: &str = "tokens";
const SYNTAX_UNIT: &str = "nodos";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpdToken {
    pub image: u64,
    pub line: usize,
    pub end_line: usize,
}

#[derive(Debug)]
pub enum DuplicateUnits {
    Tokens(Vec<CpdToken>),
    Syntax(Vec<SyntaxNode>),
    Repeated(Vec<RepeatedBlock>),
}

#[derive(Debug)]
pub struct PooledFile {
    pub identity: FileIdentity,
    pub limit: usize,
    pub units: DuplicateUnits,
}

impl PooledFile {
    pub fn package_scope(&self) -> Option<&Path> {
        match self.units {
            DuplicateUnits::Syntax(_) => self.identity.absolute.parent(),
            DuplicateUnits::Tokens(_) | DuplicateUnits::Repeated(_) => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Suppression {
    Unchanged,
    Off,
    On,
}

#[derive(Debug, Clone, Copy)]
struct Placement {
    file: usize,
    line: usize,
    end_line: usize,
}

#[derive(Debug)]
struct CloneGroup {
    size: usize,
    content: u64,
    copies: Vec<Placement>,
}

pub fn duplicate_units(
    root: Node<'_>,
    source: &[u8],
    style: DuplicateStyle,
    limit: usize,
) -> Option<DuplicateUnits> {
    match style {
        DuplicateStyle::Unmeasured => None,
        DuplicateStyle::Sonar => Some(DuplicateUnits::Repeated(sonar::repeated_blocks(
            root, source, limit,
        ))),
        DuplicateStyle::CpdJava => Some(DuplicateUnits::Tokens(java::tokens(root, source))),
        DuplicateStyle::CpdPython => python::tokens(source).map(DuplicateUnits::Tokens),
        DuplicateStyle::Dupl => go_syntax::nodes(root, source).map(DuplicateUnits::Syntax),
    }
}

pub fn find_duplicates(files: Vec<PooledFile>) -> Vec<Diagnostic> {
    let (repeated, mut files): (Vec<PooledFile>, Vec<PooledFile>) = files
        .into_iter()
        .partition(|file| matches!(file.units, DuplicateUnits::Repeated(_)));
    let mut diagnostics: Vec<Diagnostic> = repeated.iter().flat_map(repeated_diagnostics).collect();
    files.sort_by(|left, right| left.identity.absolute.cmp(&right.identity.absolute));
    diagnostics.extend(pooled_diagnostics(files));
    diagnostics
}

fn repeated_diagnostics(file: &PooledFile) -> Vec<Diagnostic> {
    let DuplicateUnits::Repeated(blocks) = &file.units else {
        return Vec::new();
    };
    let mut occurrences: HashMap<u64, usize> = HashMap::new();
    blocks
        .iter()
        .map(|block| {
            let occurrence = occurrences.entry(block.content).or_default();
            *occurrence = occurrence.saturating_add(1);
            let what = match block.kind {
                RepeatedKind::Function => "Esta función repite la",
                RepeatedKind::Branch => "Esta rama repite la",
            };
            Diagnostic {
                path: file.identity.display.clone(),
                line: block.line,
                column: 1,
                end_line: block.end_line,
                rule: RuleId::DuplicateCode,
                message: format!("{what} de la línea {}", block.original_line),
                value: block.lines,
                limit: file.limit,
                fingerprint: format!(
                    "duplicate-code:{}:{:016x}#{occurrence}",
                    file.identity.relative, block.content
                ),
                relative_path: file.identity.relative.clone(),
                absolute_path: file.identity.absolute.clone(),
            }
        })
        .collect()
}

fn pooled_diagnostics(files: Vec<PooledFile>) -> Vec<Diagnostic> {
    let Some(threshold) = files.iter().map(|file| file.limit.saturating_add(1)).min() else {
        return Vec::new();
    };
    let mut token_files = Vec::new();
    let mut syntax_files = Vec::new();
    let identities: Vec<(FileIdentity, usize)> = files
        .into_iter()
        .map(|file| {
            match file.units {
                DuplicateUnits::Tokens(tokens) => token_files.push(tokens),
                DuplicateUnits::Syntax(nodes) => syntax_files.push(nodes),
                DuplicateUnits::Repeated(_) => {}
            }
            (file.identity, file.limit)
        })
        .collect();
    let (groups, unit) = if syntax_files.is_empty() {
        (cpd_groups(token_files, threshold), TOKEN_UNIT)
    } else {
        (dupl::clone_groups(syntax_files, threshold), SYNTAX_UNIT)
    };
    report(&identities, &groups, unit)
}

fn cpd_groups(files: Vec<Vec<CpdToken>>, tile: usize) -> Vec<CloneGroup> {
    let pool = cpd::Pool::new(files);
    pool.matches(tile)
        .into_iter()
        .map(|found| CloneGroup {
            size: found.tokens,
            content: pool.content_hash(found.marks.first().copied(), found.tokens),
            copies: found
                .marks
                .iter()
                .filter_map(|&mark| pool.span(mark, found.tokens))
                .map(|span| Placement {
                    file: span.file,
                    line: span.line,
                    end_line: span.end_line,
                })
                .collect(),
        })
        .collect()
}

fn report(identities: &[(FileIdentity, usize)], groups: &[CloneGroup], unit: &str) -> Vec<Diagnostic> {
    let mut occurrences: HashMap<(usize, u64), usize> = HashMap::new();
    let mut diagnostics = Vec::new();
    for group in groups {
        for (position, copy) in group.copies.iter().enumerate() {
            let Some((identity, limit)) = identities
                .get(copy.file)
                .filter(|&&(_, limit)| group.size > limit)
            else {
                continue;
            };
            let occurrence = occurrences.entry((copy.file, group.content)).or_default();
            *occurrence = occurrence.saturating_add(1);
            let others = group
                .copies
                .iter()
                .enumerate()
                .filter(|&(other, _)| other != position)
                .filter_map(|(_, other)| {
                    identities
                        .get(other.file)
                        .map(|(other_identity, _)| format!("{}:{}", other_identity.display, other.line))
                })
                .collect::<Vec<_>>()
                .join(", ");
            diagnostics.push(Diagnostic {
                path: identity.display.clone(),
                line: copy.line,
                column: 1,
                end_line: copy.end_line,
                rule: RuleId::DuplicateCode,
                message: format!(
                    "Este bloque de {} {unit} está repetido en {others} (máximo {limit})",
                    group.size
                ),
                value: group.size,
                limit: *limit,
                fingerprint: format!(
                    "duplicate-code:{}:{:016x}#{occurrence}",
                    identity.relative, group.content
                ),
                relative_path: identity.relative.clone(),
                absolute_path: identity.absolute.clone(),
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
