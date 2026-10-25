use std::collections::HashMap;
use std::path::PathBuf;

use crate::analysis::FileAnalysis;
use crate::deploy::violation_diagnostics;
use crate::fix::Fix;
use crate::i18n::{FunctionName, Message};
use crate::limits::Limits;
use crate::metrics::FunctionMetrics;
use crate::syntax::ANONYMOUS_FUNCTION_IDENTITY;

mod families;
mod id;
mod lints;
pub use lints::finding_diagnostics;

pub use families::{
    ComposeRule, DockerfileRule, EslintRule, GoRule, JavaRule, MigrationRule, PythonRule, ShellRule,
    TaintRule,
};
pub use id::RuleId;

const FIRST_LINE: usize = 1;
const FIRST_COLUMN: usize = 1;
const MAGIC_NUMBER_COUNT: usize = 1;
const NO_MAGIC_NUMBERS: usize = 0;

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
    pub fix: Option<Fix>,
}

struct Finding<'function> {
    rule: RuleId,
    value: usize,
    limit: usize,
    function: Option<&'function FunctionMetrics>,
}

pub fn evaluate(file: &FileIdentity, analysis: &FileAnalysis, limits: &Limits) -> Vec<Diagnostic> {
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
        .chain(lints::lint_diagnostics(file, analysis))
        .chain(violation_diagnostics(file, &analysis.violations, limits))
        .collect()
}

fn nesting_diagnostics<'analysis>(
    file: &'analysis FileIdentity,
    analysis: &'analysis FileAnalysis,
    limits: &Limits,
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
            fix: None,
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
            fix: None,
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
            fix: None,
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
            fix: None,
        }
    })
}

fn function_findings<'function>(
    function: &'function FunctionMetrics,
    limits: &Limits,
) -> impl Iterator<Item = Finding<'function>> + use<'function> {
    let limits = *limits;
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
        fix: None,
    }
}

fn message(finding: &Finding<'_>, name: &str) -> Message {
    let Finding { value, limit, .. } = *finding;
    let function = shown_function_name(name);
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
        | RuleId::CommentedCode
        | RuleId::UnusedImport
        | RuleId::RedundantImport
        | RuleId::StarImport
        | RuleId::RedundantModifier
        | RuleId::FinalParameter
        | RuleId::FinalClass
        | RuleId::Migration(_)
        | RuleId::Dockerfile(_)
        | RuleId::Compose(_)
        | RuleId::CommittedSecret
        | RuleId::VulnerableDependency
        | RuleId::Python(_)
        | RuleId::Eslint(_)
        | RuleId::Shell(_)
        | RuleId::Go(_)
        | RuleId::Java(_)
        | RuleId::Taint(_) => Message::Rule(finding.rule),
    }
}

fn shown_function_name(identity: &str) -> FunctionName {
    if identity == ANONYMOUS_FUNCTION_IDENTITY {
        FunctionName::Anonymous
    } else {
        FunctionName::Named(identity.to_owned())
    }
}

#[cfg(test)]
mod tests;
