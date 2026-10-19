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
    value: usize,
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
            value: function.parameters.unwrap_or_default(),
            limit: limits.parameters.filter(|_| function.parameters.is_some()),
        },
    ];
    measurements.into_iter().filter_map(move |measurement| {
        let limit = measurement.limit.filter(|&limit| measurement.value > limit)?;
        Some(Diagnostic {
            path: path.to_owned(),
            line: function.line,
            column: function.column,
            rule: measurement.rule,
            message: function_message(measurement.rule, &function.name, measurement.value, limit),
            value: measurement.value,
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
mod tests {
    use super::{Limits, RuleId, evaluate};
    use crate::analysis::FileAnalysis;
    use crate::language::Language;
    use crate::metrics::FunctionMetrics;

    fn oversized_function(parameters: Option<usize>) -> FunctionMetrics {
        FunctionMetrics {
            name: "process".to_owned(),
            line: 3,
            column: 1,
            lines: 500,
            cyclomatic: 50,
            cognitive: 50,
            parameters,
        }
    }

    #[test]
    fn reports_every_limit_exceeded_with_its_value() {
        let analysis = FileAnalysis {
            file_lines: 900,
            functions: vec![oversized_function(Some(9))],
        };
        let diagnostics = evaluate("a.go", &analysis, Limits::strict(Language::Go));
        let rules: Vec<RuleId> = diagnostics.iter().map(|diagnostic| diagnostic.rule).collect();
        assert_eq!(
            rules,
            [
                RuleId::FileLength,
                RuleId::FunctionLength,
                RuleId::CyclomaticComplexity,
                RuleId::CognitiveComplexity,
                RuleId::ParameterCount,
            ]
        );
        let messages: Vec<&str> = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect();
        assert_eq!(
            messages,
            [
                "El archivo tiene 900 líneas (máximo 400)",
                "La función `process` tiene 500 líneas (máximo 80)",
                "La función `process` tiene complejidad ciclomática 50 (máximo 10)",
                "La función `process` tiene complejidad cognitiva 50 (máximo 15)",
                "La función `process` recibe 9 parámetros (máximo 4)",
            ]
        );
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| diagnostic.value > diagnostic.limit)
        );
    }

    #[test]
    fn skips_rules_the_language_does_not_enforce() {
        let analysis = FileAnalysis {
            file_lines: 900,
            functions: vec![oversized_function(None)],
        };
        let diagnostics = evaluate("a.py", &analysis, Limits::strict(Language::Python));
        let rules: Vec<RuleId> = diagnostics.iter().map(|diagnostic| diagnostic.rule).collect();
        assert_eq!(rules, [RuleId::CyclomaticComplexity]);
    }

    #[test]
    fn values_at_the_limit_pass() {
        let function = FunctionMetrics {
            name: "edge".to_owned(),
            line: 1,
            column: 1,
            lines: 80,
            cyclomatic: 8,
            cognitive: 15,
            parameters: Some(5),
        };
        let analysis = FileAnalysis {
            file_lines: 500,
            functions: vec![function],
        };
        assert!(evaluate("A.java", &analysis, Limits::strict(Language::Java)).is_empty());
    }

    #[test]
    fn tests_relax_only_what_the_standard_relaxes() {
        let analysis = FileAnalysis {
            file_lines: 10,
            functions: vec![oversized_function(Some(9))],
        };
        let typescript = evaluate(
            "a.test.ts",
            &analysis,
            Limits::strict_for(Language::TypeScript, true),
        );
        assert!(
            typescript
                .iter()
                .all(|diagnostic| diagnostic.rule != RuleId::FunctionLength)
        );
        assert_eq!(typescript.len(), 3);
        let python = evaluate(
            "tests/a.py",
            &analysis,
            Limits::strict_for(Language::Python, true),
        );
        assert_eq!(python.len(), 1);
        let go = evaluate("a_test.go", &analysis, Limits::strict_for(Language::Go, true));
        assert_eq!(go.len(), 4);
    }

    #[test]
    fn rule_ids_are_kebab_case() {
        assert_eq!(RuleId::FileLength.as_str(), "file-length");
        assert_eq!(RuleId::FunctionLength.as_str(), "function-length");
        assert_eq!(RuleId::CyclomaticComplexity.as_str(), "cyclomatic-complexity");
        assert_eq!(RuleId::CognitiveComplexity.as_str(), "cognitive-complexity");
        assert_eq!(RuleId::ParameterCount.as_str(), "parameter-count");
        let analysis = FileAnalysis {
            file_lines: 1,
            functions: Vec::new(),
        };
        assert!(evaluate("a.ts", &analysis, Limits::strict(Language::Tsx)).is_empty());
    }
}
