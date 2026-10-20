use super::{LimitOverride, Limits, Preset};
use crate::language::Language;
use crate::rules::RuleId;

#[test]
fn strict_is_the_standard_of_each_language() {
    let java = Preset::Strict.limits(Language::Java, false);
    assert_eq!(java.get(RuleId::CyclomaticComplexity), Some(8));
    assert_eq!(java.get(RuleId::FileLength), Some(500));
    let python = Preset::Strict.limits(Language::Python, false);
    assert_eq!(python.get(RuleId::FunctionLength), None);
    assert_eq!(python.get(RuleId::ParameterCount), Some(6));
    assert_eq!(
        Preset::Strict
            .limits(Language::Go, false)
            .get(RuleId::ParameterCount),
        Some(4)
    );
    assert_eq!(
        Preset::Strict
            .limits(Language::Tsx, false)
            .get(RuleId::FunctionLength),
        Some(120)
    );
}

#[test]
fn looser_presets_are_uniform_across_languages() {
    for language in [
        Language::Go,
        Language::Java,
        Language::Python,
        Language::JavaScript,
    ] {
        assert_eq!(
            Preset::Recommended
                .limits(language, false)
                .get(RuleId::CognitiveComplexity),
            Some(20)
        );
        assert_eq!(
            Preset::Minimal
                .limits(language, false)
                .get(RuleId::CyclomaticComplexity),
            Some(25)
        );
    }
}

#[test]
fn tests_relax_only_what_the_standard_relaxes() {
    let typescript = Preset::Strict.limits(Language::TypeScript, true);
    assert_eq!(typescript.get(RuleId::FunctionLength), None);
    assert_eq!(typescript.get(RuleId::ParameterCount), Some(4));
    assert_eq!(
        Preset::Recommended
            .limits(Language::Python, true)
            .get(RuleId::ParameterCount),
        None
    );
    assert_eq!(
        Preset::Strict.limits(Language::Java, true),
        Preset::Strict
            .limits(Language::Java, false)
            .with_rule(RuleId::MagicNumber, None)
            .with_rule(RuleId::DuplicateCode, None),
        "en los tests de Java sólo se apagan los números mágicos y CPD (includeTests=false), como en el estándar"
    );
    assert_eq!(
        Preset::Strict
            .limits(Language::Java, false)
            .get(RuleId::DuplicateCode),
        Some(99),
        "CPD del estándar marca desde 100 tokens"
    );
    assert_eq!(typescript.get(RuleId::MagicNumber), None);
    assert_eq!(
        Preset::Minimal
            .limits(Language::Go, false)
            .get(RuleId::MagicNumber),
        None
    );
}

#[test]
fn overrides_parse_and_replace_a_single_limit() {
    let parsed: LimitOverride = "cyclomatic-complexity=0".parse().expect("es válido");
    assert_eq!(
        parsed,
        LimitOverride {
            rule: RuleId::CyclomaticComplexity,
            value: 0
        }
    );
    assert!("cyclomatic-complexity".parse::<LimitOverride>().is_err());
    assert!("unknown=3".parse::<LimitOverride>().is_err());
    assert!("parameter-count=-1".parse::<LimitOverride>().is_err());
    let limits = Limits::default()
        .with_override(parsed)
        .with_rule(RuleId::FileLength, Some(3));
    assert_eq!(limits.get(RuleId::CyclomaticComplexity), Some(0));
    assert_eq!(limits.get(RuleId::FileLength), Some(3));
    assert_eq!(
        limits.with_rule(RuleId::FileLength, None).get(RuleId::FileLength),
        None
    );
}

#[test]
fn rust_measures_only_what_clippy_measures() {
    for preset in [Preset::Strict, Preset::Recommended, Preset::Minimal] {
        let rust = preset.limits(Language::Rust, false);
        for rule in [
            RuleId::FileLength,
            RuleId::CyclomaticComplexity,
            RuleId::MagicNumber,
            RuleId::ShortName,
            RuleId::DuplicateCode,
            RuleId::CommentedCode,
        ] {
            assert_eq!(rust.get(rule), None, "{rule:?} en {preset:?}");
        }
    }
    let strict = Preset::Strict.limits(Language::Rust, false);
    assert_eq!(strict.get(RuleId::FunctionLength), Some(80));
    assert_eq!(strict.get(RuleId::CognitiveComplexity), Some(15));
    assert_eq!(strict.get(RuleId::ParameterCount), Some(4));
    assert_eq!(strict.get(RuleId::NestingDepth), Some(4));
}
