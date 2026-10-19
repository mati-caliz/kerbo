use std::path::Path;

use super::Config;
use crate::language::Language;
use crate::rules::RuleId;
use crate::test_support::TempTree;

const CONFIG: &str = r#"
preset = "recommended"

[[limits]]
rule = "cyclomatic-complexity"
max = 12
reason = "el equipo acordó 12"

[[limits]]
rule = "function-length"
max = 200
language = "java"
reason = "los mappers generados son largos"

[[exceptions]]
paths = ["legacy/**"]
rule = "cognitive-complexity"
reason = "se reescribe en el primer trimestre"

[[exceptions]]
paths = ["vendor/*.ts"]
reason = "código de terceros"

[[ignore]]
paths = ["generated/**"]
reason = "lo genera el compilador de protos"
"#;

fn load(contents: &str) -> Result<Config, String> {
    let tree = TempTree::new(&format!("config-{}", contents.len()));
    tree.write("kerbo.toml", contents);
    tree.write("src/deep/a.ts", "");
    Config::load(None, &tree.path().join("src/deep")).map_err(|error| error.to_string())
}

#[test]
fn applies_preset_adjustments_and_exceptions_in_order() {
    let config = load(CONFIG).expect("la configuración es válida");
    assert_eq!(config.deviations(), 5);
    assert!(
        !config.root().ends_with("deep"),
        "la raíz es donde está kerbo.toml"
    );
    let java = config.limits_for(Path::new("src/A.java"), Language::Java, false);
    assert_eq!(java.get(RuleId::CyclomaticComplexity), Some(12));
    assert_eq!(java.get(RuleId::FunctionLength), Some(200));
    let go = config.limits_for(Path::new("src/a.go"), Language::Go, false);
    assert_eq!(
        go.get(RuleId::FunctionLength),
        Some(150),
        "el ajuste de java no toca go"
    );
    let legacy = config.limits_for(Path::new("legacy/old.go"), Language::Go, false);
    assert_eq!(legacy.get(RuleId::CognitiveComplexity), None);
    assert_eq!(legacy.get(RuleId::CyclomaticComplexity), Some(12));
    let vendored = config.limits_for(Path::new("vendor/lib.ts"), Language::TypeScript, false);
    assert!(RuleId::ALL.into_iter().all(|rule| vendored.get(rule).is_none()));
    assert!(config.is_ignored(Path::new("generated/api/client.go")));
    assert!(!config.is_ignored(Path::new("src/generated.go")));
}

#[test]
fn defaults_to_strict_without_a_file() {
    let tree = TempTree::new("config-none");
    let config = Config::load(None, tree.path()).expect("sin archivo usa el preset estricto");
    assert_eq!(config.deviations(), 0);
    assert_eq!(config.root(), tree.path());
    let limits = config.limits_for(Path::new("a.java"), Language::Java, false);
    assert_eq!(limits.get(RuleId::CyclomaticComplexity), Some(8));
}

#[test]
fn rejects_adjustments_without_reason_and_unknown_fields() {
    let missing = load("[[limits]]\nrule = \"parameter-count\"\nmax = 9\nreason = \"  \"\n");
    assert!(missing.is_err_and(|error| error.contains("reason")));
    assert!(load("[[ignore]]\npaths = [\"a\"]\n").is_err());
    assert!(load("prest = \"strict\"\n").is_err());
    assert!(load("[[limits]]\nrule = \"nope\"\nreason = \"x\"\n").is_err());
    let without_rule = load("[[exceptions]]\npaths = [\"a/**\"]\nmax = 3\nreason = \"x\"\n");
    assert!(without_rule.is_err_and(|error| error.contains("rule")));
    assert!(load("[[ignore]]\npaths = [\"a/[\"]\nreason = \"x\"\n").is_err());
}

#[test]
fn explicit_path_wins_and_unreadable_files_fail() {
    let tree = TempTree::new("config-explicit");
    tree.write("custom.toml", "preset = \"minimal\"\n");
    let config = Config::load(Some(&tree.path().join("custom.toml")), tree.path()).expect("es válida");
    let limits = config.limits_for(Path::new("a.go"), Language::Go, false);
    assert_eq!(limits.get(RuleId::CyclomaticComplexity), Some(25));
    assert!(Config::load(Some(&tree.path().join("missing.toml")), tree.path()).is_err());
}
