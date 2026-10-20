use std::path::{Path, PathBuf};

use super::{ScanOutcome, ScanRequest, scan};
use crate::config::Config;
use crate::language::Stack;
use crate::rules::RuleId;
use crate::test_support::TempTree;

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
    tree.write("a/x.ts", "function build(ab, bc, cd, de, ef) {}\n");
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
    let many = "function build(ab, bc, cd, de, ef) {}\n";
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
