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
        "// eslint-disable-next-line no-console\nconst x = \"eslint-disable\" as unknown as number;\nlet y: any;\nit.skip(\"a\", () => run());\ntest(\"b\", () => run());\n",
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
        "import os  # noqa: F401\n\n@pytest.mark.skip(reason=\"x\")\ndef test_a():\n    run()\n\ndef helper():\n    pytest.skip()\n",
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
        "test(\"one\", () => run());\ntest(\"two\", () => run());\n",
    );
    tree.write("src/gone.test.ts", "test(\"lost\", () => run());\n");
    tree.write("src/moved.test.ts", "test(\"keeps\", () => run());\n");
    git(tree.path(), &["add", "."]);
    git(tree.path(), &["commit", "-q", "-m", "base"]);
    tree.write(
        "src/a.ts",
        "// eslint-disable-next-line old\nexport const a = 1;\n// @ts-ignore\nexport const b: any = 2;\n",
    );
    tree.write("src/a.test.ts", "it.only(\"one\", () => run());\n");
    std::fs::remove_file(tree.path().join("src/gone.test.ts")).expect("se borra");
    std::fs::remove_file(tree.path().join("src/moved.test.ts")).expect("se mueve");
    tree.write("src/destination.test.ts", "test(\"keeps\", () => run());\n");
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
    tree.write(
        "old.test.ts",
        "test(\"a\", () => run());\ntest(\"b\", () => run());\n",
    );
    git(tree.path(), &["add", "."]);
    git(tree.path(), &["commit", "-q", "-m", "base"]);
    tree.write("old.test.ts", "test(\"a\", () => run());\n");
    tree.write("new.test.ts", "test(\"b\", () => run());\n");
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

#[test]
fn finds_tests_left_without_a_body() {
    let typescript = kinds_in(
        Language::TypeScript,
        "it(\"vacío\", () => {});\nit(\"con algo\", () => { expect(1).toBe(1); });\nit(\"expresión\", () => expect(1).toBe(1));\n",
    );
    let empty: Vec<&str> = typescript
        .iter()
        .filter(|(kind, _)| *kind == MarkerKind::EmptyTest)
        .map(|(_, text)| text.as_str())
        .collect();
    assert_eq!(empty, ["it"]);
    let java = kinds_in(
        Language::Java,
        "class ATest {\n  @Test\n  void empty() {\n    // después\n  }\n  @Test\n  void full() { assertTrue(true); }\n}\n",
    );
    assert!(java.contains(&(MarkerKind::EmptyTest, "empty".to_owned())));
    assert!(!java.contains(&(MarkerKind::EmptyTest, "full".to_owned())));
    let python = kinds_in(Language::Python, "def test_a():\n    \"\"\"doc\"\"\"\n    ...\n");
    assert!(python.contains(&(MarkerKind::EmptyTest, "test_a".to_owned())));
    let go = kinds_in(Language::Go, "package a\n\nfunc TestA(t *testing.T) {\n}\n");
    assert!(go.contains(&(MarkerKind::EmptyTest, "TestA".to_owned())));
}

#[test]
fn reports_emptied_tests_and_loosened_thresholds_in_the_change() {
    let tree = TempTree::new("guard-loosened");
    git(tree.path(), &["init", "-q"]);
    tree.write(".golangci.yml", "cyclop:\n  max-complexity: 10\n");
    tree.write(
        "a_test.go",
        "package a\n\nfunc TestA(t *testing.T) {\n\tcheck(t)\n}\n",
    );
    git(tree.path(), &["add", "."]);
    git(tree.path(), &["commit", "-q", "-m", "base"]);
    tree.write(".golangci.yml", "cyclop:\n  max-complexity: 15\n");
    tree.write("a_test.go", "package a\n\nfunc TestA(t *testing.T) {\n}\n");
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
            (".golangci.yml".to_owned(), RuleId::SuppressionAdded, 2),
            ("a_test.go".to_owned(), RuleId::TestDisabled, 3)
        ],
        "vaciar un test sin tocar su firma también cuenta"
    );
}
