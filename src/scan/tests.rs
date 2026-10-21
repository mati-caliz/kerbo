use std::path::{Path, PathBuf};

use super::{ScanOutcome, ScanRequest, scan};
use crate::cache::AnalysisCache;
use crate::config::Config;
use crate::i18n::Lang;
use crate::language::Stack;
use crate::report::{Format, render};
use crate::rules::RuleId;
use crate::test_support::TempTree;

const CACHE_BYTES: usize = 16 * 1024 * 1024;

fn scan_tree(root: &Path, roots: &[PathBuf], stacks: &[Stack]) -> ScanOutcome {
    let config = Config::load(None, root).expect("la configuración es válida");
    scan(&ScanRequest {
        roots,
        stacks,
        overrides: &[],
        config: &config,
        cache: None,
    })
}

#[test]
fn analyzes_supported_files_in_parallel_and_sorts_the_problems() {
    let tree = TempTree::new("scan-mixed");
    tree.write("b.py", "def f(a, b, c, d, e, f, g):\n    pass\n");
    tree.write(
        "a/x.ts",
        "export function build(ab, bc, cd, de, ef) {\n  return [ab, bc, cd, de, ef];\n}\n",
    );
    tree.write("notes.md", "# no es código\n");
    let outcome = scan_tree(tree.path(), &[tree.path().to_path_buf()], &[]);
    assert_eq!(outcome.files, 2);
    assert!(outcome.failures.is_empty());
    let found: Vec<(&str, RuleId)> = outcome
        .diagnostics
        .iter()
        .map(|diagnostic| (diagnostic.relative_path.as_str(), diagnostic.rule))
        .collect();
    assert_eq!(
        found,
        [
            ("a/x.ts", RuleId::ParameterCount),
            ("b.py", RuleId::ParameterCount)
        ]
    );
}

#[test]
fn walks_the_listed_hidden_folders_and_skips_the_rest() {
    let tree = TempTree::new("scan-hidden");
    tree.write(".devcontainer/Dockerfile", "FROM ubuntu:24.04\n");
    tree.write(
        ".github/docker/compose.yaml",
        "services:\n  web:\n    image: nginx:1.27\n",
    );
    tree.write(
        ".github/scripts/release.py",
        "def f(a, b, c, d, e, f, g):\n    pass\n",
    );
    tree.write(".venv/lib/site.py", "def f(a, b, c, d, e, f, g):\n    pass\n");
    tree.write(".github/.hidden.py", "def f(a, b, c, d, e, f, g):\n    pass\n");
    tree.write(".hidden.py", "def f(a, b, c, d, e, f, g):\n    pass\n");
    let outcome = scan_tree(tree.path(), &[tree.path().to_path_buf()], &[]);
    assert_eq!(outcome.files, 3);
    assert!(
        outcome
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.relative_path.starts_with(".github/")
                || diagnostic.relative_path.starts_with(".devcontainer/"))
    );
}

#[test]
fn accepts_several_roots_and_single_files() {
    let tree = TempTree::new("scan-roots");
    tree.write("one/A.java", "class A { void f() {} }\n");
    tree.write("two.go", "package two\n");
    let outcome = scan_tree(
        tree.path(),
        &[tree.path().join("one"), tree.path().join("two.go")],
        &[],
    );
    assert_eq!(outcome.files, 2);
    assert!(outcome.diagnostics.is_empty());
    assert!(scan_tree(tree.path(), &[], &[]).diagnostics.is_empty());
    let only_go = scan_tree(tree.path(), &[tree.path().to_path_buf()], &[Stack::Go]);
    assert_eq!(only_go.files, 1);
}

#[test]
fn follows_the_project_configuration() {
    let tree = TempTree::new("scan-config");
    tree.write(
        "kerbo.toml",
        "[[ignore]]\npaths = [\"gen/**\"]\nreason = \"generado\"\n\n[[exceptions]]\npaths = [\"old/**\"]\nrule = \"parameter-count\"\nmax = 9\nreason = \"heredado\"\n",
    );
    let many = "export function build(ab, bc, cd, de, ef) {\n  return [ab, bc, cd, de, ef];\n}\n";
    tree.write("gen/client.ts", many);
    tree.write("old/legacy.ts", many);
    tree.write("src/new.ts", many);
    let outcome = scan_tree(&tree.path().join("src"), &[tree.path().to_path_buf()], &[]);
    assert_eq!(outcome.files, 2, "gen/ queda afuera");
    let reported: Vec<&str> = outcome
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.relative_path.as_str())
        .collect();
    assert_eq!(reported, ["src/new.ts"]);
}

#[test]
fn recognizes_tests_passed_as_single_files() {
    let tree = TempTree::new("scan-single-test");
    let magic = "package a\n\nfunc f() int { return 5 }\n";
    tree.write("a_test.go", magic);
    tree.write("a.go", magic);
    let outcome = scan_tree(
        tree.path(),
        &[tree.path().join("a_test.go"), tree.path().join("a.go")],
        &[],
    );
    let reported: Vec<&str> = outcome
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.relative_path.as_str())
        .collect();
    assert_eq!(reported, ["a.go"], "los números mágicos de un test no cuentan");
}

fn fake_key() -> String {
    ["AKIA", "QYLPMN5HHHFPZAM2"].concat()
}

#[test]
fn finds_committed_secrets_in_every_file_also_hidden_ones() {
    let tree = TempTree::new("scan-secrets");
    let key = fake_key();
    tree.write(".env", &format!("AWS_ACCESS_KEY_ID={key}\n"));
    tree.write("src/config.py", &format!("KEY = \"{key}\"\n"));
    tree.write(
        ".idea/scripts/deploy.py",
        "def deploy(a, b, c, d, e, f, g):\n    pass\n",
    );
    tree.write("image.png", &key);
    let outcome = scan_tree(tree.path(), &[tree.path().to_path_buf()], &[]);
    let found: Vec<(&str, RuleId)> = outcome
        .diagnostics
        .iter()
        .map(|diagnostic| (diagnostic.relative_path.as_str(), diagnostic.rule))
        .collect();
    assert_eq!(
        found,
        [
            (".env", RuleId::CommittedSecret),
            ("src/config.py", RuleId::CommittedSecret)
        ]
    );
    assert!(
        outcome
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.fingerprint.contains(&key))
    );
    for format in [
        Format::Text,
        Format::Json,
        Format::Sarif,
        Format::Agent,
        Format::AgentJson,
    ] {
        let mut rendered = Vec::new();
        render(&outcome.diagnostics, format, Lang::Spanish, &mut rendered).expect("se escribe el reporte");
        assert!(!String::from_utf8_lossy(&rendered).contains(&key), "{format:?}");
    }
    let only_go = scan_tree(tree.path(), &[tree.path().to_path_buf()], &[Stack::Go]);
    assert!(only_go.diagnostics.is_empty());
}

#[test]
fn cached_secrets_come_back_masked_and_identical() {
    let tree = TempTree::new("scan-secrets-cache");
    let key = fake_key();
    tree.write("deploy/.env", &format!("AWS_ACCESS_KEY_ID={key}\n"));
    tree.write("src/keys.py", &format!("KEY = \"{key}\"\n"));
    let config = Config::load(None, tree.path()).expect("la configuración es válida");
    let cache = AnalysisCache::open(tree.path().join("cache"), CACHE_BYTES);
    let run = || {
        let outcome = scan(&ScanRequest {
            roots: &[tree.path().to_path_buf()],
            stacks: &[],
            overrides: &[],
            config: &config,
            cache: Some(&cache),
        });
        outcome
            .diagnostics
            .into_iter()
            .map(|diagnostic| (diagnostic.fingerprint, diagnostic.line, diagnostic.message))
            .collect::<Vec<_>>()
    };
    let fresh = run();
    assert_eq!(fresh.len(), 2);
    assert_eq!(run(), fresh);
    let reopened = AnalysisCache::open(tree.path().join("cache"), CACHE_BYTES);
    let profile = crate::secrets::path_profile("x.env").expect("no está excluido");
    let stored =
        reopened.secrets(reopened.secrets_key(&profile, format!("AWS_ACCESS_KEY_ID={key}\n").as_bytes()));
    let stored = stored.expect("quedó guardado en la caché");
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].preview, "AKIA");
}

#[test]
fn secrets_can_be_excused_per_path_with_a_reason() {
    let tree = TempTree::new("scan-secrets-excused");
    let key = fake_key();
    tree.write("fixtures/keys.txt", &format!("aws = {key}\n"));
    tree.write("fixtures/keys.py", &format!("AWS = \"{key}\"\n"));
    tree.write(
        "kerbo.toml",
        "[[exceptions]]\npaths = [\"fixtures/**\"]\nrule = \"committed-secret\"\nreason = \"claves falsas de prueba\"\n",
    );
    let outcome = scan_tree(tree.path(), &[tree.path().to_path_buf()], &[]);
    assert!(
        outcome
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.rule != RuleId::CommittedSecret),
        "{:?}",
        outcome.diagnostics
    );
}

#[test]
fn checks_secrets_in_the_text_the_editor_sends() {
    let tree = TempTree::new("scan-secrets-text");
    let config = Config::load(None, tree.path()).expect("la configuración es válida");
    let text = format!("AWS_ACCESS_KEY_ID={}\n", fake_key());
    let found = super::check_text(&tree.path().join(".env"), text.as_bytes(), &config).expect("se revisa");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].rule, RuleId::CommittedSecret);
}
