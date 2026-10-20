use std::path::PathBuf;

use tree_sitter::Parser;

use super::{FileIdentity, RuleId, evaluate};
use crate::analysis::{FileAnalysis, analyze};
use crate::i18n::{Lang, Problem};
use crate::language::Language;
use crate::limits::{Limits, Preset};
use crate::metrics::FunctionMetrics;

fn function(name: &str, parameters: Option<usize>) -> FunctionMetrics {
    FunctionMetrics {
        name: name.to_owned(),
        line: 3,
        column: 1,
        end_line: 503,
        lines: Some(500),
        cyclomatic: Some(50),
        cognitive: Some(50),
        parameters,
    }
}

fn file(relative: &str) -> FileIdentity {
    FileIdentity {
        display: format!("./{relative}"),
        relative: relative.to_owned(),
        absolute: PathBuf::from(relative),
    }
}

#[test]
fn reports_every_limit_exceeded_with_its_value() {
    let analysis = FileAnalysis {
        nesting: Vec::new(),
        magic_numbers: Vec::new(),
        short_names: Vec::new(),
        duplicate_units: None,
        commented_code: Vec::new(),
        lints: Vec::new(),
        violations: Vec::new(),
        file_lines: 900,
        functions: vec![function("process", Some(9))],
    };
    let diagnostics = evaluate(
        &file("a.go"),
        &analysis,
        Preset::Strict.limits(Language::Go, false),
    );
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
    let messages: Vec<String> = diagnostics
        .iter()
        .map(|diagnostic| Lang::Spanish.message(&diagnostic.message))
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
    let file_length = diagnostics.first().expect("hay diagnósticos");
    assert_eq!(
        (file_length.line, file_length.end_line, file_length.path.as_str()),
        (1, 900, "./a.go")
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.value > diagnostic.limit)
    );
}

#[test]
fn fingerprints_ignore_lines_and_number_repeated_symbols() {
    let analysis = FileAnalysis {
        nesting: Vec::new(),
        magic_numbers: Vec::new(),
        short_names: Vec::new(),
        duplicate_units: None,
        commented_code: Vec::new(),
        lints: Vec::new(),
        violations: Vec::new(),
        file_lines: 10,
        functions: vec![
            function("handle", None),
            function("handle", None),
            function("other", None),
        ],
    };
    let limits = Limits::default().with_rule(RuleId::CyclomaticComplexity, Some(1));
    let fingerprints: Vec<String> = evaluate(&file("src/a.ts"), &analysis, limits)
        .into_iter()
        .map(|diagnostic| diagnostic.fingerprint)
        .collect();
    assert_eq!(
        fingerprints,
        [
            "cyclomatic-complexity:src/a.ts:handle#1",
            "cyclomatic-complexity:src/a.ts:handle#2",
            "cyclomatic-complexity:src/a.ts:other#1",
        ]
    );
}

#[test]
fn skips_rules_without_limit_or_measure() {
    let analysis = FileAnalysis {
        nesting: Vec::new(),
        magic_numbers: Vec::new(),
        short_names: Vec::new(),
        duplicate_units: None,
        commented_code: Vec::new(),
        lints: Vec::new(),
        violations: Vec::new(),
        file_lines: 900,
        functions: vec![function("f", None)],
    };
    let diagnostics = evaluate(
        &file("a.py"),
        &analysis,
        Preset::Strict.limits(Language::Python, false),
    );
    let rules: Vec<RuleId> = diagnostics.iter().map(|diagnostic| diagnostic.rule).collect();
    assert_eq!(rules, [RuleId::CyclomaticComplexity]);
}

#[test]
fn values_at_the_limit_pass() {
    let edge = FunctionMetrics {
        name: "edge".to_owned(),
        line: 1,
        column: 1,
        end_line: 80,
        lines: Some(80),
        cyclomatic: Some(8),
        cognitive: Some(15),
        parameters: Some(5),
    };
    let analysis = FileAnalysis {
        nesting: Vec::new(),
        magic_numbers: Vec::new(),
        short_names: Vec::new(),
        duplicate_units: None,
        commented_code: Vec::new(),
        lints: Vec::new(),
        violations: Vec::new(),
        file_lines: 500,
        functions: vec![edge],
    };
    assert!(
        evaluate(
            &file("A.java"),
            &analysis,
            Preset::Strict.limits(Language::Java, false)
        )
        .is_empty()
    );
}

#[test]
fn anonymous_functions_keep_their_fingerprint_and_translate_their_name() {
    let mut parser = Parser::new();
    parser
        .set_language(&Language::TypeScript.grammar().expect("tiene gramática"))
        .expect("la gramática carga");
    let source = "export default function (a, b, c, d, e) { return a; }\n";
    let limits = Limits::default().with_rule(RuleId::ParameterCount, Some(1));
    let analysis =
        analyze(&mut parser, Language::TypeScript, source.as_bytes(), limits).expect("el archivo se parsea");
    let diagnostics = evaluate(&file("src/a.ts"), &analysis, limits);
    let anonymous = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.rule == RuleId::ParameterCount)
        .expect("la función anónima se pasa de parámetros");
    assert_eq!(anonymous.fingerprint, "parameter-count:src/a.ts:(anónima)#1");
    assert_eq!(
        Lang::English.message(&anonymous.message),
        "The function `(anonymous)` takes 5 parameters (maximum 1)"
    );
    assert_eq!(
        Lang::Spanish.message(&anonymous.message),
        "La función `(anónima)` recibe 5 parámetros (máximo 1)"
    );
}

#[test]
fn rule_ids_round_trip_and_describe_themselves() {
    for rule in RuleId::ALL {
        assert_eq!(rule.as_str().parse::<RuleId>(), Ok(rule));
        assert_ne!(Lang::Spanish.describe(rule), Lang::English.describe(rule));
    }
    assert_eq!(
        "nope".parse::<RuleId>(),
        Err(Problem::UnknownRule("nope".to_owned()))
    );
}
