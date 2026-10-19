use std::str::FromStr;

use serde::Serialize;

use crate::analysis::FileAnalysis;
use crate::language::Language;
use crate::metrics::FunctionMetrics;

const FIRST_LINE: usize = 1;
const FIRST_COLUMN: usize = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LimitOverride {
    pub rule: RuleId,
    pub value: usize,
}

impl FromStr for LimitOverride {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (rule_name, value) = text
            .split_once('=')
            .ok_or_else(|| format!("se esperaba regla=valor y llegó `{text}`"))?;
        let rule = RuleId::ALL
            .into_iter()
            .find(|rule| rule.as_str() == rule_name.trim())
            .ok_or_else(|| format!("no existe la regla `{}`", rule_name.trim()))?;
        let value = value
            .trim()
            .parse()
            .map_err(|_| format!("`{}` no es un número entero no negativo", value.trim()))?;
        Ok(Self { rule, value })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    file_lines: Option<usize>,
    function_lines: Option<usize>,
    cyclomatic: Option<usize>,
    cognitive: Option<usize>,
    parameters: Option<usize>,
}

const GO_LIMITS: Limits = Limits {
    file_lines: Some(400),
    function_lines: Some(80),
    cyclomatic: Some(10),
    cognitive: Some(15),
    parameters: Some(4),
};

const PYTHON_LIMITS: Limits = Limits {
    file_lines: None,
    function_lines: None,
    cyclomatic: Some(10),
    cognitive: None,
    parameters: Some(6),
};

const JAVA_LIMITS: Limits = Limits {
    file_lines: Some(500),
    function_lines: Some(80),
    cyclomatic: Some(8),
    cognitive: Some(15),
    parameters: Some(5),
};

const ECMASCRIPT_LIMITS: Limits = Limits {
    file_lines: Some(400),
    function_lines: Some(120),
    cyclomatic: Some(10),
    cognitive: Some(15),
    parameters: Some(4),
};

impl Limits {
    pub const fn strict_for(language: Language, is_test: bool) -> Self {
        let limits = Self::strict(language);
        if !is_test {
            return limits;
        }
        match language {
            Language::TypeScript | Language::Tsx | Language::JavaScript => Self {
                function_lines: None,
                ..limits
            },
            Language::Python => Self {
                parameters: None,
                ..limits
            },
            Language::Go | Language::Java => limits,
        }
    }

    pub const fn with_override(self, limit_override: LimitOverride) -> Self {
        let value = Some(limit_override.value);
        match limit_override.rule {
            RuleId::FileLength => Self {
                file_lines: value,
                ..self
            },
            RuleId::FunctionLength => Self {
                function_lines: value,
                ..self
            },
            RuleId::CyclomaticComplexity => Self {
                cyclomatic: value,
                ..self
            },
            RuleId::CognitiveComplexity => Self {
                cognitive: value,
                ..self
            },
            RuleId::ParameterCount => Self {
                parameters: value,
                ..self
            },
        }
    }

    pub const fn strict(language: Language) -> Self {
        match language {
            Language::Go => GO_LIMITS,
            Language::Python => PYTHON_LIMITS,
            Language::Java => JAVA_LIMITS,
            Language::TypeScript | Language::Tsx | Language::JavaScript => ECMASCRIPT_LIMITS,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Diagnostic {
    pub path: String,
    pub line: usize,
    pub column: usize,
    pub rule: RuleId,
    pub message: String,
    pub value: usize,
    pub limit: usize,
}

struct Measurement {
    rule: RuleId,
    value: Option<usize>,
    limit: Option<usize>,
}

pub fn evaluate(path: &str, analysis: &FileAnalysis, limits: Limits) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    if let Some(limit) = limits.file_lines.filter(|&limit| analysis.file_lines > limit) {
        diagnostics.push(Diagnostic {
            path: path.to_owned(),
            line: FIRST_LINE,
            column: FIRST_COLUMN,
            rule: RuleId::FileLength,
            message: format!("El archivo tiene {} líneas (máximo {limit})", analysis.file_lines),
            value: analysis.file_lines,
            limit,
        });
    }
    for function in &analysis.functions {
        diagnostics.extend(evaluate_function(path, function, limits));
    }
    diagnostics
}

fn evaluate_function<'function>(
    path: &'function str,
    function: &'function FunctionMetrics,
    limits: Limits,
) -> impl Iterator<Item = Diagnostic> + 'function {
    let measurements = [
        Measurement {
            rule: RuleId::FunctionLength,
            value: function.lines,
            limit: limits.function_lines,
        },
        Measurement {
            rule: RuleId::CyclomaticComplexity,
            value: function.cyclomatic,
            limit: limits.cyclomatic,
        },
        Measurement {
            rule: RuleId::CognitiveComplexity,
            value: function.cognitive,
            limit: limits.cognitive,
        },
        Measurement {
            rule: RuleId::ParameterCount,
            value: function.parameters,
            limit: limits.parameters,
        },
    ];
    measurements.into_iter().filter_map(move |measurement| {
        let value = measurement.value?;
        let limit = measurement.limit.filter(|&limit| value > limit)?;
        Some(Diagnostic {
            path: path.to_owned(),
            line: function.line,
            column: function.column,
            rule: measurement.rule,
            message: function_message(measurement.rule, &function.name, value, limit),
            value,
            limit,
        })
    })
}

fn function_message(rule: RuleId, name: &str, value: usize, limit: usize) -> String {
    match rule {
        RuleId::FileLength | RuleId::FunctionLength => {
            format!("La función `{name}` tiene {value} líneas (máximo {limit})")
        }
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
