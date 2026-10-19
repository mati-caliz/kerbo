use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::analysis::FileAnalysis;
use crate::limits::Limits;
use crate::metrics::FunctionMetrics;

const FIRST_LINE: usize = 1;
const FIRST_COLUMN: usize = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuleId {
    FileLength,
    FunctionLength,
    CyclomaticComplexity,
    CognitiveComplexity,
    ParameterCount,
}

impl RuleId {
    pub const ALL: [Self; 5] = [
        Self::FileLength,
        Self::FunctionLength,
        Self::CyclomaticComplexity,
        Self::CognitiveComplexity,
        Self::ParameterCount,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FileLength => "file-length",
            Self::FunctionLength => "function-length",
            Self::CyclomaticComplexity => "cyclomatic-complexity",
            Self::CognitiveComplexity => "cognitive-complexity",
            Self::ParameterCount => "parameter-count",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::FileLength => "El archivo supera el máximo de líneas",
            Self::FunctionLength => "La función supera el máximo de líneas",
            Self::CyclomaticComplexity => "La función tiene demasiados caminos posibles",
            Self::CognitiveComplexity => "La función es difícil de leer: demasiadas ramas anidadas",
            Self::ParameterCount => "La función recibe demasiados parámetros",
        }
    }
}

impl FromStr for RuleId {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|rule| rule.as_str() == text)
            .ok_or_else(|| format!("no existe la regla `{text}`"))
    }
}

#[derive(Debug, Clone)]
pub struct FileIdentity {
    pub display: String,
    pub relative: String,
    pub absolute: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub path: String,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub rule: RuleId,
    pub message: String,
    pub value: usize,
    pub limit: usize,
    pub fingerprint: String,
    #[serde(skip)]
    pub relative_path: String,
    #[serde(skip)]
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
        .collect()
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

fn message(finding: &Finding<'_>, name: &str) -> String {
    let Finding { value, limit, .. } = *finding;
    match finding.rule {
        RuleId::FileLength => format!("El archivo tiene {value} líneas (máximo {limit})"),
        RuleId::FunctionLength => format!("La función `{name}` tiene {value} líneas (máximo {limit})"),
        RuleId::CyclomaticComplexity => {
            format!("La función `{name}` tiene complejidad ciclomática {value} (máximo {limit})")
        }
        RuleId::CognitiveComplexity => {
            format!("La función `{name}` tiene complejidad cognitiva {value} (máximo {limit})")
        }
        RuleId::ParameterCount => format!("La función `{name}` recibe {value} parámetros (máximo {limit})"),
    }
}

#[cfg(test)]
mod tests;
