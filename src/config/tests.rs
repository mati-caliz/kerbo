use std::path::Path;

use super::Config;
use crate::i18n::Lang;
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
    Config::load(None, &tree.path().join("src/deep")).map_err(|problem| Lang::Spanish.problem(&problem))
}

#[test]
fn reads_the_language_of_the_messages() {
    assert_eq!(
        load("lang = \"en\"\n").map(|config| config.lang()),
        Ok(Some(Lang::English))
    );
    assert_eq!(
        load("lang = \"es\"\n").map(|config| config.lang()),
        Ok(Some(Lang::Spanish))
    );
    assert_eq!(load("").map(|config| config.lang()), Ok(None));
    assert!(load("lang = \"fr\"\n").is_err());
}

#[test]
fn reads_the_cache_size_cap() {
    assert_eq!(
        load("[cache]\nmax_megabytes = 64\n").map(|config| config.cache_max_megabytes()),
        Ok(Some(64))
    );
    assert_eq!(load("").map(|config| config.cache_max_megabytes()), Ok(None));
    assert!(load("[cache]\nmax_mb = 64\n").is_err());
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

#[test]
fn reads_the_go_platform_and_build_tags() {
    let config = load("[go]\ngoos = \"windows\"\ngoarch = \"arm64\"\nbuild_tags = [\"integration\"]\n")
        .expect("la configuración es válida");
    let target = config.go_target();
    assert_eq!(target.goos.as_deref(), Some("windows"));
    assert_eq!(target.goarch.as_deref(), Some("arm64"));
    assert_eq!(target.build_tags, ["integration"]);
    assert!(load("[go]\ntags = []\n").is_err());
}
