use std::path::Path;
use std::sync::Arc;

use tree_sitter::Parser;

use crate::deploy::{self, Violation};
use crate::duplicates::{DuplicateUnits, duplicate_units};
use crate::language::Language;
use crate::limits::Limits;
use crate::lines::LineMap;
use crate::lint::{LintFinding, find_lints};
use crate::macro_rules::{CrateMacros, MacroScope};
use crate::metrics::commented::{CommentedCode, find_commented_code};
use crate::metrics::magic::{MagicNumber, find_magic_numbers};
use crate::metrics::names::{ShortName, find_short_names};
use crate::metrics::nesting::{NestingFinding, find_deep_nesting};
use crate::metrics::{FunctionMetrics, measure_clippy_functions, measure_functions};
use crate::profile::MetricsStyle;
use crate::rules::RuleId;
use crate::shell;
use crate::syntax::rust_parse_text;

#[derive(Debug)]
pub struct FileAnalysis {
    pub file_lines: usize,
    pub functions: Vec<FunctionMetrics>,
    pub nesting: Vec<NestingFinding>,
    pub magic_numbers: Vec<MagicNumber>,
    pub short_names: Vec<ShortName>,
    pub duplicate_units: Option<DuplicateUnits>,
    pub commented_code: Vec<CommentedCode>,
    pub lints: Vec<LintFinding>,
    pub violations: Vec<Violation>,
}

impl FileAnalysis {
    const fn of_violations(violations: Vec<Violation>) -> Self {
        Self {
            file_lines: 0,
            functions: Vec::new(),
            nesting: Vec::new(),
            magic_numbers: Vec::new(),
            short_names: Vec::new(),
            duplicate_units: None,
            commented_code: Vec::new(),
            lints: Vec::new(),
            violations,
        }
    }
}

#[derive(Debug)]
pub struct ParseFailed;

#[derive(Debug, Clone, Copy)]
pub struct SourceUnit<'unit> {
    pub language: Language,
    pub source: &'unit [u8],
    pub path: Option<&'unit Path>,
    pub crate_macros: Option<&'unit Arc<CrateMacros>>,
}

#[cfg(test)]
pub fn analyze(
    parser: &mut Parser,
    language: Language,
    source: &[u8],
    limits: Limits,
) -> Result<FileAnalysis, ParseFailed> {
    let unit = SourceUnit {
        language,
        source,
        path: None,
        crate_macros: None,
    };
    analyze_unit(parser, unit, limits)
}

pub fn analyze_unit(
    parser: &mut Parser,
    unit: SourceUnit<'_>,
    limits: Limits,
) -> Result<FileAnalysis, ParseFailed> {
    let SourceUnit { language, source, .. } = unit;
    if let Some(violations) =
        deploy::analyze(parser, language, source).or_else(|| shell::analyze(parser, unit))
    {
        return violations.map(FileAnalysis::of_violations);
    }
    let profile = language.profile().ok_or(ParseFailed)?;
    let tree = if language == Language::Rust {
        parser.parse(&*rust_parse_text(source), None)
    } else {
        parser.parse(source, None)
    }
    .ok_or(ParseFailed)?;
    let root = tree.root_node();
    let line_map = LineMap::new(root, source, profile);
    let file_lines = line_map.count(0, line_map.total_rows().saturating_sub(1), profile.file_lines);
    let file_lines = if line_map.total_rows() == 0 { 0 } else { file_lines };
    let scope = (profile.metrics == MetricsStyle::Clippy)
        .then(|| MacroScope::new(unit.crate_macros.cloned(), unit.path, source));
    Ok(FileAnalysis {
        file_lines,
        functions: scope.as_ref().map_or_else(
            || measure_functions(root, source, profile, &line_map),
            |scope| measure_clippy_functions(root, scope),
        ),
        nesting: find_deep_nesting(
            root,
            profile.nesting,
            limits.get(RuleId::NestingDepth),
            scope.as_ref(),
        ),
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
        lints: find_lints(root, unit, profile.lints, limits),
        duplicate_units: limits
            .get(RuleId::DuplicateCode)
            .and_then(|limit| duplicate_units(root, source, profile.duplicates, limit)),
        violations: if language == Language::Python {
            deploy::alembic_violations(root, source)
        } else {
            Vec::new()
        },
    })
}
