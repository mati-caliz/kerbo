use super::{LimitOverride, Limits, RuleId, evaluate};
use crate::analysis::FileAnalysis;
use crate::language::Language;
use crate::metrics::FunctionMetrics;

fn oversized_function(parameters: Option<usize>) -> FunctionMetrics {
    FunctionMetrics {
        name: "process".to_owned(),
        line: 3,
        column: 1,
        lines: Some(500),
        cyclomatic: Some(50),
        cognitive: Some(50),
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
        lines: Some(80),
        cyclomatic: Some(8),
        cognitive: Some(15),
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
    let analysis = FileAnalysis {
        file_lines: 10,
        functions: vec![oversized_function(Some(2))],
    };
    let limits = Limits::strict(Language::Python)
        .with_override(LimitOverride {
            rule: RuleId::ParameterCount,
            value: 1,
        })
        .with_override(LimitOverride {
            rule: RuleId::FileLength,
            value: 5,
        })
        .with_override(LimitOverride {
            rule: RuleId::FunctionLength,
            value: 5,
        })
        .with_override(LimitOverride {
            rule: RuleId::CognitiveComplexity,
            value: 5,
        })
        .with_override(LimitOverride {
            rule: RuleId::CyclomaticComplexity,
            value: 100,
        });
    let rules: Vec<RuleId> = evaluate("a.py", &analysis, limits)
        .iter()
        .map(|diagnostic| diagnostic.rule)
        .collect();
    assert_eq!(
        rules,
        [
            RuleId::FileLength,
            RuleId::FunctionLength,
            RuleId::CognitiveComplexity,
            RuleId::ParameterCount
        ]
    );
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
