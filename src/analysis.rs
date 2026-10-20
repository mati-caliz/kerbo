use std::fmt;

use tree_sitter::Parser;

use crate::duplicates::{DuplicateUnits, duplicate_units};
use crate::language::Language;
use crate::limits::Limits;
use crate::lines::LineMap;
use crate::metrics::commented::{CommentedCode, find_commented_code};
use crate::metrics::magic::{MagicNumber, find_magic_numbers};
use crate::metrics::names::{ShortName, find_short_names};
use crate::metrics::nesting::{NestingFinding, find_deep_nesting};
use crate::metrics::{FunctionMetrics, measure_functions};
use crate::rules::RuleId;

#[derive(Debug)]
pub struct FileAnalysis {
    pub file_lines: usize,
    pub functions: Vec<FunctionMetrics>,
    pub nesting: Vec<NestingFinding>,
    pub magic_numbers: Vec<MagicNumber>,
    pub short_names: Vec<ShortName>,
    pub duplicate_units: Option<DuplicateUnits>,
    pub commented_code: Vec<CommentedCode>,
}

#[derive(Debug)]
pub struct ParseFailed;

impl fmt::Display for ParseFailed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("tree-sitter no devolvió un árbol")
    }
}

pub fn analyze(
    parser: &mut Parser,
    language: Language,
    source: &[u8],
    limits: Limits,
) -> Result<FileAnalysis, ParseFailed> {
    let tree = parser.parse(source, None).ok_or(ParseFailed)?;
    let root = tree.root_node();
    let profile = language.profile();
    let line_map = LineMap::new(root, source, profile);
    let file_lines = line_map.count(0, line_map.total_rows().saturating_sub(1), profile.file_lines);
    let file_lines = if line_map.total_rows() == 0 { 0 } else { file_lines };
    Ok(FileAnalysis {
        file_lines,
        functions: measure_functions(root, source, profile, &line_map),
        nesting: find_deep_nesting(root, profile.nesting, limits.get(RuleId::NestingDepth)),
        magic_numbers: if limits.get(RuleId::MagicNumber).is_some() {
            find_magic_numbers(root, source, profile.magic_numbers)
        } else {
            Vec::new()
        },
        short_names: if limits.get(RuleId::ShortName).is_some() {
            find_short_names(root, source, profile.short_names)
        } else {
            Vec::new()
        },
        commented_code: if limits.get(RuleId::CommentedCode).is_some() {
            find_commented_code(root, source, profile.commented_code)
        } else {
            Vec::new()
        },
        duplicate_units: limits
            .get(RuleId::DuplicateCode)
            .and_then(|limit| duplicate_units(root, source, profile.duplicates, limit)),
    })
}
