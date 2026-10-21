use crate::language::Language;
use crate::limits::Preset;
use crate::rules::{EslintRule, RuleId};

const CHECKED_BY_TYPESCRIPT: &[EslintRule] = &[EslintRule::NoDupeKeys];
const STRICT_ONLY: &[EslintRule] = &[EslintRule::NoNonNullAssertion];
const SUGGESTIONS: &[EslintRule] = &[
    EslintRule::NoVar,
    EslintRule::PreferConst,
    EslintRule::NoUnusedExpressions,
    EslintRule::NoExplicitAny,
    EslintRule::PreferRestParams,
    EslintRule::NoEmpty,
    EslintRule::NoUselessEscape,
    EslintRule::NoThisAlias,
    EslintRule::NoEmptyObjectType,
    EslintRule::NoCaseDeclarations,
];

pub fn relaxed_by(preset: Preset, language: Language) -> Vec<RuleId> {
    let by_language: &[EslintRule] = match language {
        Language::TypeScript | Language::Tsx => CHECKED_BY_TYPESCRIPT,
        Language::JavaScript
        | Language::Go
        | Language::Python
        | Language::Java
        | Language::Rust
        | Language::Sql
        | Language::Dockerfile
        | Language::Compose
        | Language::Shell => &[],
    };
    let by_preset: &[&[EslintRule]] = match preset {
        Preset::Strict => &[],
        Preset::Recommended => &[STRICT_ONLY],
        Preset::Minimal => &[STRICT_ONLY, SUGGESTIONS],
    };
    by_preset
        .iter()
        .copied()
        .chain([by_language])
        .flatten()
        .copied()
        .map(RuleId::Eslint)
        .collect()
}
