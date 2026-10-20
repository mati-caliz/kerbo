use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::analysis::FileAnalysis;
use crate::i18n::{Message, Problem};
use crate::limits::Limits;
use crate::metrics::FunctionMetrics;

const FIRST_LINE: usize = 1;
const FIRST_COLUMN: usize = 1;
const MAGIC_NUMBER_COUNT: usize = 1;
const NO_MAGIC_NUMBERS: usize = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuleId {
    FileLength,
    FunctionLength,
    CyclomaticComplexity,
    CognitiveComplexity,
    ParameterCount,
    SuppressionAdded,
    TestDisabled,
    TestRemoved,
    TypeEscape,
    NestingDepth,
    MagicNumber,
    ShortName,
    DuplicateCode,
    CommentedCode,
}

impl RuleId {
    pub const ALL: [Self; 14] = [
        Self::FileLength,
        Self::FunctionLength,
        Self::CyclomaticComplexity,
        Self::CognitiveComplexity,
        Self::ParameterCount,
        Self::SuppressionAdded,
        Self::TestDisabled,
        Self::TestRemoved,
        Self::TypeEscape,
        Self::NestingDepth,
        Self::MagicNumber,
        Self::ShortName,
        Self::DuplicateCode,
        Self::CommentedCode,
    ];

    pub const fn index(self) -> usize {
        match self {
            Self::FileLength => 0,
            Self::FunctionLength => 1,
            Self::CyclomaticComplexity => 2,
            Self::CognitiveComplexity => 3,
            Self::ParameterCount => 4,
            Self::SuppressionAdded => 5,
            Self::TestDisabled => 6,
            Self::TestRemoved => 7,
            Self::TypeEscape => 8,
            Self::NestingDepth => 9,
            Self::MagicNumber => 10,
            Self::ShortName => 11,
            Self::DuplicateCode => 12,
            Self::CommentedCode => 13,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FileLength => "file-length",
            Self::FunctionLength => "function-length",
            Self::CyclomaticComplexity => "cyclomatic-complexity",
            Self::CognitiveComplexity => "cognitive-complexity",
            Self::ParameterCount => "parameter-count",
            Self::SuppressionAdded => "suppression-added",
            Self::TestDisabled => "test-disabled",
            Self::TestRemoved => "test-removed",
            Self::TypeEscape => "type-escape",
            Self::NestingDepth => "nesting-depth",
            Self::MagicNumber => "magic-number",
            Self::ShortName => "short-name",
            Self::DuplicateCode => "duplicate-code",
            Self::CommentedCode => "commented-code",
        }
    }
}

impl FromStr for RuleId {
    type Err = Problem;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|rule| rule.as_str() == text)
            .ok_or_else(|| Problem::UnknownRule(text.to_owned()))
    }
}

#[derive(Debug, Clone)]
pub struct FileIdentity {
    pub display: String,
    pub relative: String,
    pub absolute: PathBuf,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub path: String,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub rule: RuleId,
    pub message: Message,
    pub value: usize,
    pub limit: usize,
    pub fingerprint: String,
    pub relative_path: String,
    pub absolute_path: PathBuf,
}

struct Finding<'function> {
    rule: RuleId,
    value: usize,
    limit: usize,
    function: Option<&'function FunctionMetrics>,
}

pub fn evaluate(file: &FileIdentity, analysis: &FileAnalysis, limits: Limits) -> Vec<Diagnostic> {
    let file_finding = limits
        .get(RuleId::FileLength)
        .filter(|&limit| analysis.file_lines > limit)
        .map(|limit| Finding {
            rule: RuleId::FileLength,
            value: analysis.file_lines,
            limit,
            function: None,
        });
    let function_findings = analysis
        .functions
        .iter()
        .flat_map(|function| function_findings(function, limits));
    let mut occurrences: HashMap<(RuleId, &str), usize> = HashMap::new();
    file_finding
        .into_iter()
        .chain(function_findings)
        .map(|finding| {
            let symbol = finding.function.map_or("", |function| function.name.as_str());
            let occurrence = occurrences.entry((finding.rule, symbol)).or_default();
            *occurrence = occurrence.saturating_add(1);
            diagnostic(file, &finding, analysis.file_lines, *occurrence)
        })
        .chain(nesting_diagnostics(file, analysis, limits))
        .chain(magic_number_diagnostics(file, analysis))
        .chain(short_name_diagnostics(file, analysis))
        .chain(commented_code_diagnostics(file, analysis))
        .collect()
}

fn nesting_diagnostics<'analysis>(
    file: &'analysis FileIdentity,
    analysis: &'analysis FileAnalysis,
    limits: Limits,
) -> impl Iterator<Item = Diagnostic> + 'analysis {
    let limit = limits.get(RuleId::NestingDepth);
    let mut occurrences: HashMap<usize, usize> = HashMap::new();
    analysis.nesting.iter().filter_map(move |found| {
        let limit = limit.filter(|&limit| found.depth > limit)?;
        let occurrence = occurrences.entry(found.depth).or_default();
        *occurrence = occurrence.saturating_add(1);
        Some(Diagnostic {
            path: file.display.clone(),
            line: found.line,
            column: found.column,
            end_line: found.line,
            rule: RuleId::NestingDepth,
            message: Message::NestedTooDeep {
                depth: found.depth,
                limit,
            },
            value: found.depth,
            limit,
            fingerprint: format!("nesting-depth:{}:{}#{occurrence}", file.relative, found.depth),
            relative_path: file.relative.clone(),
            absolute_path: file.absolute.clone(),
        })
    })
}

fn magic_number_diagnostics<'analysis>(
    file: &'analysis FileIdentity,
    analysis: &'analysis FileAnalysis,
) -> impl Iterator<Item = Diagnostic> + 'analysis {
    let mut occurrences: HashMap<&str, usize> = HashMap::new();
    analysis.magic_numbers.iter().map(move |found| {
        let occurrence = occurrences.entry(found.text.as_str()).or_default();
        *occurrence = occurrence.saturating_add(1);
        Diagnostic {
            path: file.display.clone(),
            line: found.line,
            column: found.column,
            end_line: found.line,
            rule: RuleId::MagicNumber,
            message: Message::UnnamedNumber {
                number: found.text.clone(),
            },
            value: MAGIC_NUMBER_COUNT,
            limit: NO_MAGIC_NUMBERS,
            fingerprint: format!("magic-number:{}:{}#{occurrence}", file.relative, found.text),
            relative_path: file.relative.clone(),
            absolute_path: file.absolute.clone(),
        }
    })
}

fn short_name_diagnostics<'analysis>(
    file: &'analysis FileIdentity,
    analysis: &'analysis FileAnalysis,
) -> impl Iterator<Item = Diagnostic> + 'analysis {
    let mut occurrences: HashMap<&str, usize> = HashMap::new();
    analysis.short_names.iter().map(move |found| {
        let occurrence = occurrences.entry(found.name.as_str()).or_default();
        *occurrence = occurrence.saturating_add(1);
        Diagnostic {
            path: file.display.clone(),
            line: found.line,
            column: found.column,
            end_line: found.line,
            rule: RuleId::ShortName,
            message: Message::ShortName {
                name: found.name.clone(),
            },
            value: found.name.chars().count(),
            limit: NO_MAGIC_NUMBERS,
            fingerprint: format!("short-name:{}:{}#{occurrence}", file.relative, found.name),
            relative_path: file.relative.clone(),
            absolute_path: file.absolute.clone(),
        }
    })
}

fn commented_code_diagnostics<'analysis>(
    file: &'analysis FileIdentity,
    analysis: &'analysis FileAnalysis,
) -> impl Iterator<Item = Diagnostic> + 'analysis {
    let mut occurrences: HashMap<&str, usize> = HashMap::new();
    analysis.commented_code.iter().map(move |found| {
        let occurrence = occurrences.entry(found.text.as_str()).or_default();
        *occurrence = occurrence.saturating_add(1);
        Diagnostic {
            path: file.display.clone(),
            line: found.line,
            column: found.column,
            end_line: found.line,
            rule: RuleId::CommentedCode,
            message: Message::Rule(RuleId::CommentedCode),
            value: MAGIC_NUMBER_COUNT,
            limit: NO_MAGIC_NUMBERS,
            fingerprint: format!("commented-code:{}:{}#{occurrence}", file.relative, found.text),
            relative_path: file.relative.clone(),
            absolute_path: file.absolute.clone(),
        }
    })
}

fn function_findings(function: &FunctionMetrics, limits: Limits) -> impl Iterator<Item = Finding<'_>> {
    let measured = [
        (RuleId::FunctionLength, function.lines),
        (RuleId::CyclomaticComplexity, function.cyclomatic),
        (RuleId::CognitiveComplexity, function.cognitive),
        (RuleId::ParameterCount, function.parameters),
    ];
    measured.into_iter().filter_map(move |(rule, value)| {
        let value = value?;
        let limit = limits.get(rule).filter(|&limit| value > limit)?;
        Some(Finding {
            rule,
            value,
            limit,
            function: Some(function),
        })
    })
}

fn diagnostic(
    file: &FileIdentity,
    finding: &Finding<'_>,
    file_lines: usize,
    occurrence: usize,
) -> Diagnostic {
    let (line, column, end_line, symbol) = finding.function.map_or_else(
        || (FIRST_LINE, FIRST_COLUMN, file_lines.max(FIRST_LINE), ""),
        |function| {
            (
                function.line,
                function.column,
                function.end_line,
                function.name.as_str(),
            )
        },
    );
    Diagnostic {
        path: file.display.clone(),
        line,
        column,
        end_line,
        rule: finding.rule,
        message: message(finding, symbol),
        value: finding.value,
        limit: finding.limit,
        fingerprint: format!(
            "{}:{}:{symbol}#{occurrence}",
            finding.rule.as_str(),
            file.relative
        ),
        relative_path: file.relative.clone(),
        absolute_path: file.absolute.clone(),
    }
}

fn message(finding: &Finding<'_>, name: &str) -> Message {
    let Finding { value, limit, .. } = *finding;
    let function = name.to_owned();
    match finding.rule {
        RuleId::FileLength => Message::FileTooLong { lines: value, limit },
        RuleId::FunctionLength => Message::FunctionTooLong {
            function,
            lines: value,
            limit,
        },
        RuleId::CyclomaticComplexity => Message::CyclomaticComplexity {
            function,
            value,
            limit,
        },
        RuleId::CognitiveComplexity => Message::CognitiveComplexity {
            function,
            value,
            limit,
        },
        RuleId::ParameterCount => Message::TooManyParameters {
            function,
            count: value,
            limit,
        },
        RuleId::SuppressionAdded
        | RuleId::TestDisabled
        | RuleId::TestRemoved
        | RuleId::TypeEscape
        | RuleId::NestingDepth
        | RuleId::MagicNumber
        | RuleId::ShortName
        | RuleId::DuplicateCode
        | RuleId::CommentedCode => Message::Rule(finding.rule),
    }
}

#[cfg(test)]
mod tests;
