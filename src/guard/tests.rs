use std::path::Path;
use std::process::Command;

use tree_sitter::Parser;

use super::markers::{MarkerKind, find_markers};
use super::{GuardRequest, inspect};
use crate::config::Config;
use crate::diff::ChangedLines;
use crate::language::Language;
use crate::rules::RuleId;
use crate::test_support::TempTree;

fn kinds_in(language: Language, source: &str) -> Vec<(MarkerKind, String)> {
    let mut parser = Parser::new();
    parser
        .set_language(&language.grammar())
        .expect("la gramática carga");
    let tree = parser.parse(source, None).expect("se parsea");
    find_markers(tree.root_node(), source.as_bytes(), language.profile())
        .into_iter()
        .map(|marker| (marker.kind, marker.text))
        .collect()
}

#[test]
fn finds_suppressions_disabled_tests_and_escapes_in_typescript() {
    let found = kinds_in(
        Language::TypeScript,
        "// eslint-disable-next-line no-console\nconst x = \"eslint-disable\" as unknown as number;\nlet y: any;\nit.skip(\"a\", () => {});\ntest(\"b\", () => {});\n",
    );
    assert_eq!(
        found,
        [
            (MarkerKind::Suppression, "eslint-disable".to_owned()),
            (
                MarkerKind::TypeEscape,
                "\"eslint-disable\" as unknown as number".to_owned()
            ),
            (MarkerKind::TypeEscape, "any".to_owned()),
            (MarkerKind::DisabledTest, "it.skip".to_owned()),
            (MarkerKind::TestDefinition, "it.skip".to_owned()),
            (MarkerKind::TestDefinition, "test".to_owned()),
        ],
        "un eslint-disable dentro de un string no cuenta"
    );
}

#[test]
fn finds_markers_in_python_java_and_go() {
    let python = kinds_in(
        Language::Python,
        "import os  # noqa: F401\n\n@pytest.mark.skip(reason=\"x\")\ndef test_a():\n    pass\n\ndef helper():\n    pytest.skip()\n",
    );
    assert_eq!(
        python.iter().map(|(kind, _)| *kind).collect::<Vec<_>>(),
        [
            MarkerKind::Suppression,
            MarkerKind::DisabledTest,
            MarkerKind::TestDefinition,
            MarkerKind::DisabledTest
        ]
    );
    let java = kinds_in(
        Language::Java,
        "class T {\n  @Test @Disabled void a() {}\n  @org.junit.Ignore @Test void b() {}\n  @SuppressWarnings(\"x\") void c() {} // NOPMD\n}\n",
    );
    let java_kinds: Vec<MarkerKind> = java.iter().map(|(kind, _)| *kind).collect();
    assert_eq!(
        java_kinds
            .iter()
            .filter(|&&kind| kind == MarkerKind::TestDefinition)
            .count(),
        2
    );
    assert_eq!(
        java_kinds
            .iter()
            .filter(|&&kind| kind == MarkerKind::DisabledTest)
            .count(),
        2
    );
    assert_eq!(
        java_kinds
            .iter()
            .filter(|&&kind| kind == MarkerKind::Suppression)
            .count(),
        2
    );
    let go = kinds_in(
        Language::Go,
        "package a\n\nfunc TestA(t *testing.T) {\n\tt.Skip()\n} //nolint:errcheck\n",
    );
    assert_eq!(
        go.iter().map(|(kind, _)| *kind).collect::<Vec<_>>(),
        [
            MarkerKind::TestDefinition,
            MarkerKind::DisabledTest,
            MarkerKind::Suppression
        ]
    );
}

fn git(directory: &Path, arguments: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(directory)
        .args(["-c", "user.name=kerbo", "-c", "user.email=kerbo@example.com"])
        .args(arguments)
        .status()
        .expect("git está instalado");
    assert!(status.success(), "git {arguments:?}");
}

#[test]
fn reports_only_what_the_change_adds() {
    let tree = TempTree::new("guard-git");
    git(tree.path(), &["init", "-q"]);
    tree.write(
        "src/a.ts",
        "// eslint-disable-next-line old\nexport const a = 1;\n",
    );
    tree.write(
        "src/a.test.ts",
        "test(\"one\", () => {});\ntest(\"two\", () => {});\n",
    );
    tree.write("src/gone.test.ts", "test(\"lost\", () => {});\n");
    tree.write("src/moved.test.ts", "test(\"keeps\", () => {});\n");
    git(tree.path(), &["add", "."]);
    git(tree.path(), &["commit", "-q", "-m", "base"]);
    tree.write(
        "src/a.ts",
        "// eslint-disable-next-line old\nexport const a = 1;\n// @ts-ignore\nexport const b: any = 2;\n",
    );
    tree.write("src/a.test.ts", "it.only(\"one\", () => {});\n");
    std::fs::remove_file(tree.path().join("src/gone.test.ts")).expect("se borra");
    std::fs::remove_file(tree.path().join("src/moved.test.ts")).expect("se mueve");
    tree.write("src/destination.test.ts", "test(\"keeps\", () => {});\n");
    let root = tree.path().canonicalize().expect("existe");
    let changes = ChangedLines::since("HEAD", &root).expect("git responde");
    let config = Config::load(None, &root).expect("sin archivo");
    let request = GuardRequest {
        changes: &changes,
        config: &config,
        project_root: &root,
        stacks: &[],
    };
    let found: Vec<(String, RuleId, usize)> = inspect(&request)
        .into_iter()
        .map(|diagnostic| (diagnostic.relative_path, diagnostic.rule, diagnostic.line))
        .collect();
    assert_eq!(
        found,
        [
            ("src/a.test.ts".to_owned(), RuleId::TestDisabled, 1),
            ("src/a.test.ts".to_owned(), RuleId::TestRemoved, 1),
            ("src/a.ts".to_owned(), RuleId::SuppressionAdded, 3),
            ("src/a.ts".to_owned(), RuleId::TypeEscape, 4),
            ("src/gone.test.ts".to_owned(), RuleId::TestRemoved, 1),
            ("src/moved.test.ts".to_owned(), RuleId::TestRemoved, 1),
        ],
        "la supresión vieja de la línea 1 no se reporta; con el neto en baja se marca todo lo que perdió tests"
    );
    tree.write(
        "kerbo.toml",
        "[[exceptions]]\npaths = [\"src/*.ts\"]\nrule = \"type-escape\"\nreason = \"migración\"\n",
    );
    let tuned = Config::load(None, &root).expect("es válida");
    let tuned_request = GuardRequest {
        changes: &changes,
        config: &tuned,
        project_root: &root,
        stacks: &[],
    };
    assert!(
        inspect(&tuned_request)
            .iter()
            .all(|diagnostic| diagnostic.rule != RuleId::TypeEscape)
    );
}

#[test]
fn moving_tests_between_files_is_not_removing_them() {
    let tree = TempTree::new("guard-move");
    git(tree.path(), &["init", "-q"]);
    tree.write("old.test.ts", "test(\"a\", () => {});\ntest(\"b\", () => {});\n");
    git(tree.path(), &["add", "."]);
    git(tree.path(), &["commit", "-q", "-m", "base"]);
    tree.write("old.test.ts", "test(\"a\", () => {});\n");
    tree.write("new.test.ts", "test(\"b\", () => {});\n");
    let root = tree.path().canonicalize().expect("existe");
    let changes = ChangedLines::since("HEAD", &root).expect("git responde");
    let config = Config::load(None, &root).expect("sin archivo");
    let request = GuardRequest {
        changes: &changes,
        config: &config,
        project_root: &root,
        stacks: &[],
    };
    assert!(inspect(&request).is_empty());
}
