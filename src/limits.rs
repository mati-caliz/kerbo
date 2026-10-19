use std::str::FromStr;

use clap::ValueEnum;
use serde::Deserialize;

use crate::language::Language;
use crate::rules::RuleId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Limits {
    file_lines: Option<usize>,
    function_lines: Option<usize>,
    cyclomatic: Option<usize>,
    cognitive: Option<usize>,
    parameters: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LimitOverride {
    pub rule: RuleId,
    pub value: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Preset {
    #[default]
    Strict,
    Recommended,
    Minimal,
}

const GO_STRICT: Limits = Limits::new([Some(400), Some(80), Some(10), Some(15), Some(4)]);
const PYTHON_STRICT: Limits = Limits::new([None, None, Some(10), None, Some(6)]);
const JAVA_STRICT: Limits = Limits::new([Some(500), Some(80), Some(8), Some(15), Some(5)]);
const ECMASCRIPT_STRICT: Limits = Limits::new([Some(400), Some(120), Some(10), Some(15), Some(4)]);
const RECOMMENDED: Limits = Limits::new([Some(750), Some(150), Some(15), Some(20), Some(6)]);
const MINIMAL: Limits = Limits::new([Some(1500), Some(300), Some(25), Some(30), Some(8)]);

impl Limits {
    const fn new(
        [file_lines, function_lines, cyclomatic, cognitive, parameters]: [Option<usize>; 5],
    ) -> Self {
        Self {
            file_lines,
            function_lines,
            cyclomatic,
            cognitive,
            parameters,
        }
    }

    pub const fn get(self, rule: RuleId) -> Option<usize> {
        match rule {
            RuleId::FileLength => self.file_lines,
            RuleId::FunctionLength => self.function_lines,
            RuleId::CyclomaticComplexity => self.cyclomatic,
            RuleId::CognitiveComplexity => self.cognitive,
            RuleId::ParameterCount => self.parameters,
        }
    }

    pub const fn with_rule(self, rule: RuleId, max: Option<usize>) -> Self {
        match rule {
            RuleId::FileLength => Self {
                file_lines: max,
                ..self
            },
            RuleId::FunctionLength => Self {
                function_lines: max,
                ..self
            },
            RuleId::CyclomaticComplexity => Self {
                cyclomatic: max,
                ..self
            },
            RuleId::CognitiveComplexity => Self {
                cognitive: max,
                ..self
            },
            RuleId::ParameterCount => Self {
                parameters: max,
                ..self
            },
        }
    }

    pub const fn with_override(self, limit_override: LimitOverride) -> Self {
        self.with_rule(limit_override.rule, Some(limit_override.value))
    }
}

impl Preset {
    pub const fn limits(self, language: Language, is_test: bool) -> Limits {
        let base = match self {
            Self::Strict => strict(language),
            Self::Recommended => RECOMMENDED,
            Self::Minimal => MINIMAL,
        };
        if !is_test {
            return base;
        }
        match language {
            Language::TypeScript | Language::Tsx | Language::JavaScript => {
                base.with_rule(RuleId::FunctionLength, None)
            }
            Language::Python => base.with_rule(RuleId::ParameterCount, None),
            Language::Go | Language::Java => base,
        }
    }
}

const fn strict(language: Language) -> Limits {
    match language {
        Language::Go => GO_STRICT,
        Language::Python => PYTHON_STRICT,
        Language::Java => JAVA_STRICT,
        Language::TypeScript | Language::Tsx | Language::JavaScript => ECMASCRIPT_STRICT,
    }
}

impl FromStr for LimitOverride {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (rule_name, value) = text
            .split_once('=')
            .ok_or_else(|| format!("se esperaba regla=valor y llegó `{text}`"))?;
        let rule = rule_name.trim().parse()?;
        let value = value
            .trim()
            .parse()
            .map_err(|_| format!("`{}` no es un número entero no negativo", value.trim()))?;
        Ok(Self { rule, value })
    }
}

#[cfg(test)]
mod tests;
