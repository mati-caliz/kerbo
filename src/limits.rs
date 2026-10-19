use std::str::FromStr;

use clap::ValueEnum;
use serde::Deserialize;

use crate::language::Language;
use crate::rules::RuleId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Limits {
    values: [Option<usize>; RuleId::ALL.len()],
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

const ALWAYS_REPORTED: Option<usize> = Some(0);
const GO_STRICT: [Option<usize>; 5] = [Some(400), Some(80), Some(10), Some(15), Some(4)];
const PYTHON_STRICT: [Option<usize>; 5] = [None, None, Some(10), None, Some(6)];
const JAVA_STRICT: [Option<usize>; 5] = [Some(500), Some(80), Some(8), Some(15), Some(5)];
const ECMASCRIPT_STRICT: [Option<usize>; 5] = [Some(400), Some(120), Some(10), Some(15), Some(4)];
const NESTING_STRICT: Option<usize> = Some(4);
const NESTING_RECOMMENDED: Option<usize> = Some(5);
const NESTING_MINIMAL: Option<usize> = Some(6);
const RECOMMENDED: [Option<usize>; 5] = [Some(750), Some(150), Some(15), Some(20), Some(6)];
const MINIMAL: [Option<usize>; 5] = [Some(1500), Some(300), Some(25), Some(30), Some(8)];

impl Limits {
    fn with_metrics([file, function, cyclomatic, cognitive, parameters]: [Option<usize>; 5]) -> Self {
        let mut limits = Self {
            values: [ALWAYS_REPORTED; RuleId::ALL.len()],
        };
        let metrics = [
            (RuleId::FileLength, file),
            (RuleId::FunctionLength, function),
            (RuleId::CyclomaticComplexity, cyclomatic),
            (RuleId::CognitiveComplexity, cognitive),
            (RuleId::ParameterCount, parameters),
        ];
        for (rule, max) in metrics {
            limits = limits.with_rule(rule, max);
        }
        limits
    }

    pub fn get(self, rule: RuleId) -> Option<usize> {
        self.values.get(rule.index()).copied().flatten()
    }

    pub fn with_rule(mut self, rule: RuleId, max: Option<usize>) -> Self {
        if let Some(slot) = self.values.get_mut(rule.index()) {
            *slot = max;
        }
        self
    }

    pub fn with_override(self, limit_override: LimitOverride) -> Self {
        self.with_rule(limit_override.rule, Some(limit_override.value))
    }
}

impl Preset {
    pub fn limits(self, language: Language, is_test: bool) -> Limits {
        let (metrics, nesting) = match self {
            Self::Strict => (strict(language), NESTING_STRICT),
            Self::Recommended => (RECOMMENDED, NESTING_RECOMMENDED),
            Self::Minimal => (MINIMAL, NESTING_MINIMAL),
        };
        let base = Limits::with_metrics(metrics).with_rule(RuleId::NestingDepth, nesting);
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

const fn strict(language: Language) -> [Option<usize>; 5] {
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
