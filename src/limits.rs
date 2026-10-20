use std::str::FromStr;

use clap::ValueEnum;
use serde::Deserialize;

use crate::i18n::Problem;
use crate::language::Language;
use crate::rules::RuleId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Limits {
    values: [Option<u32>; RuleId::ALL.len()],
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

const ALWAYS_REPORTED: Option<u32> = Some(0);
const GO_STRICT: [Option<usize>; 5] = [Some(400), Some(80), Some(10), Some(15), Some(4)];
const PYTHON_STRICT: [Option<usize>; 5] = [None, None, Some(10), None, Some(6)];
const JAVA_STRICT: [Option<usize>; 5] = [Some(500), Some(80), Some(8), Some(15), Some(5)];
const ECMASCRIPT_STRICT: [Option<usize>; 5] = [Some(400), Some(120), Some(10), Some(15), Some(4)];
const RUST_STRICT: [Option<usize>; 5] = [None, Some(80), None, Some(15), Some(4)];
const RUST_UNMEASURED: &[RuleId] = &[
    RuleId::FileLength,
    RuleId::CyclomaticComplexity,
    RuleId::MagicNumber,
    RuleId::ShortName,
    RuleId::DuplicateCode,
    RuleId::CommentedCode,
];
const RECOMMENDED_OFF: &[RuleId] = &[RuleId::FinalParameter];
const MINIMAL_OFF: &[RuleId] = &[
    RuleId::MagicNumber,
    RuleId::CommentedCode,
    RuleId::StarImport,
    RuleId::RedundantModifier,
    RuleId::FinalParameter,
    RuleId::FinalClass,
];
const NESTING_STRICT: Option<usize> = Some(4);
const NESTING_RECOMMENDED: Option<usize> = Some(5);
const NESTING_MINIMAL: Option<usize> = Some(6);
const DUPLICATES_STRICT: DuplicateLimits = DuplicateLimits {
    tokens: 99,
    nodes: 149,
    lines: 2,
};
const DUPLICATES_RECOMMENDED: DuplicateLimits = DuplicateLimits {
    tokens: 149,
    nodes: 224,
    lines: 4,
};

#[derive(Debug, Clone, Copy)]
struct DuplicateLimits {
    tokens: usize,
    nodes: usize,
    lines: usize,
}

impl DuplicateLimits {
    const fn for_language(self, language: Language) -> usize {
        match language {
            Language::Go => self.nodes,
            Language::Python | Language::Java | Language::Rust => self.tokens,
            Language::TypeScript | Language::Tsx | Language::JavaScript => self.lines,
        }
    }
}
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
        self.values
            .get(rule.index())
            .copied()
            .flatten()
            .and_then(|value| usize::try_from(value).ok())
    }

    pub fn with_rule(mut self, rule: RuleId, max: Option<usize>) -> Self {
        if let Some(slot) = self.values.get_mut(rule.index()) {
            *slot = max.map(|value| u32::try_from(value).unwrap_or(u32::MAX));
        }
        self
    }

    pub fn with_override(self, limit_override: LimitOverride) -> Self {
        self.with_rule(limit_override.rule, Some(limit_override.value))
    }
}

impl Preset {
    pub fn limits(self, language: Language, is_test: bool) -> Limits {
        let (metrics, nesting, duplicates) = match self {
            Self::Strict => (
                strict(language),
                NESTING_STRICT,
                Some(DUPLICATES_STRICT.for_language(language)),
            ),
            Self::Recommended => (
                RECOMMENDED,
                NESTING_RECOMMENDED,
                Some(DUPLICATES_RECOMMENDED.for_language(language)),
            ),
            Self::Minimal => (MINIMAL, NESTING_MINIMAL, None),
        };
        let base = Limits::with_metrics(metrics)
            .with_rule(RuleId::NestingDepth, nesting)
            .with_rule(RuleId::DuplicateCode, duplicates);
        let relaxed: &[RuleId] = match self {
            Self::Strict => &[],
            Self::Recommended => RECOMMENDED_OFF,
            Self::Minimal => MINIMAL_OFF,
        };
        let base = relaxed
            .iter()
            .fold(base, |limits, &rule| limits.with_rule(rule, None));
        let base = unmeasured(language)
            .iter()
            .fold(base, |limits, &rule| limits.with_rule(rule, None));
        if !is_test {
            return base;
        }
        let base = base
            .with_rule(RuleId::MagicNumber, None)
            .with_rule(RuleId::DuplicateCode, None);
        match language {
            Language::TypeScript | Language::Tsx | Language::JavaScript => {
                base.with_rule(RuleId::FunctionLength, None)
            }
            Language::Python => base.with_rule(RuleId::ParameterCount, None),
            Language::Go | Language::Java | Language::Rust => base,
        }
    }
}

const fn strict(language: Language) -> [Option<usize>; 5] {
    match language {
        Language::Go => GO_STRICT,
        Language::Python => PYTHON_STRICT,
        Language::Java => JAVA_STRICT,
        Language::TypeScript | Language::Tsx | Language::JavaScript => ECMASCRIPT_STRICT,
        Language::Rust => RUST_STRICT,
    }
}

const fn unmeasured(language: Language) -> &'static [RuleId] {
    match language {
        Language::Rust => RUST_UNMEASURED,
        Language::Go
        | Language::Python
        | Language::Java
        | Language::TypeScript
        | Language::Tsx
        | Language::JavaScript => &[],
    }
}

impl FromStr for LimitOverride {
    type Err = Problem;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (rule_name, value) = text
            .split_once('=')
            .ok_or_else(|| Problem::ExpectedRuleLimit(text.to_owned()))?;
        let rule = rule_name.trim().parse()?;
        let value = value
            .trim()
            .parse()
            .map_err(|_| Problem::NotANonNegativeInteger(value.trim().to_owned()))?;
        Ok(Self { rule, value })
    }
}

#[cfg(test)]
mod tests;
