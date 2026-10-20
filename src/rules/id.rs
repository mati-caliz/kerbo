use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::i18n::Problem;

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
    UnusedImport,
    RedundantImport,
    StarImport,
    RedundantModifier,
    FinalParameter,
    FinalClass,
}

impl RuleId {
    pub const ALL: [Self; 20] = [
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
        Self::UnusedImport,
        Self::RedundantImport,
        Self::StarImport,
        Self::RedundantModifier,
        Self::FinalParameter,
        Self::FinalClass,
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
            Self::UnusedImport => 14,
            Self::RedundantImport => 15,
            Self::StarImport => 16,
            Self::RedundantModifier => 17,
            Self::FinalParameter => 18,
            Self::FinalClass => 19,
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
            Self::UnusedImport => "unused-import",
            Self::RedundantImport => "redundant-import",
            Self::StarImport => "star-import",
            Self::RedundantModifier => "redundant-modifier",
            Self::FinalParameter => "final-parameter",
            Self::FinalClass => "final-class",
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
