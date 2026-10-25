use tree_sitter::{Node, Parser, Tree};

use super::{Fix, TextEdit, TextPoint, apply, compatible, edits_of, file_patch};
use crate::language::Language;
use crate::syntax::descendants;

fn parse(source: &str) -> Tree {
    let mut parser = Parser::new();
    parser
        .set_language(&Language::Java.grammar().expect("tiene gramática"))
        .expect("la gramática carga");
    parser.parse(source, None).expect("el archivo se parsea")
}

fn first<'tree>(tree: &'tree Tree, kind: &str) -> Node<'tree> {
    descendants(tree.root_node())
        .into_iter()
        .find(|node| node.kind() == kind)
        .expect("el nodo está")
}

fn fixed(source: &str, fix: &Fix) -> String {
    apply(&fix.edits, source).expect("el arreglo cae dentro del archivo")
}

#[test]
fn removes_the_whole_line_when_nothing_else_lives_there() {
    let source = "import a.B;\r\nimport c.D;\r\nclass A {}\r\n";
    let tree = parse(source);
    let fix = Fix::remove(first(&tree, "import_declaration"), source.as_bytes());
    assert_eq!(fixed(source, &fix), "import c.D;\r\nclass A {}\r\n");
    assert_eq!(fix.hunk, "@@ -1 +0,0 @@\n-import a.B;");
}

#[test]
fn removes_only_the_node_and_its_trailing_space_on_a_shared_line() {
    let source = "class A {\n  public static void m() {}\n}\n";
    let tree = parse(source);
    let modifiers = first(&tree, "modifiers");
    let public = modifiers.child(0).expect("public");
    let fix = Fix::remove(public, source.as_bytes());
    assert_eq!(fixed(source, &fix), "class A {\n  static void m() {}\n}\n");
    assert_eq!(
        fix.hunk,
        "@@ -2 +2 @@\n-  public static void m() {}\n+  static void m() {}"
    );
}

#[test]
fn inserts_before_a_node_and_survives_a_last_line_without_newline() {
    let source = "class A { void m(int value) {} }";
    let parsed = parse(source);
    let insertion = Fix::insert_before(first(&parsed, "integral_type"), "final ", source.as_bytes());
    assert_eq!(
        fixed(source, &insertion),
        "class A { void m(final int value) {} }"
    );
    let last = "class A {}\nimport a.B;";
    let parsed_last = parse(last);
    let removal = Fix::remove(first(&parsed_last, "import_declaration"), last.as_bytes());
    assert_eq!(fixed(last, &removal), "class A {}\n");
}

#[test]
fn rejects_edits_outside_the_text() {
    let outside = TextEdit {
        start: TextPoint { line: 5, column: 1 },
        end: TextPoint { line: 5, column: 2 },
        replacement: String::new(),
    };
    assert_eq!(apply(&[outside], "a\nb\n"), None);
    let inside_a_character = TextEdit {
        start: TextPoint { line: 1, column: 2 },
        end: TextPoint { line: 1, column: 2 },
        replacement: "x".to_owned(),
    };
    assert_eq!(apply(&[inside_a_character], "ñ"), None);
}

fn edit(line: usize, start: usize, end: usize, replacement: &str) -> Fix {
    Fix {
        edits: vec![TextEdit {
            start: TextPoint { line, column: start },
            end: TextPoint { line, column: end },
            replacement: replacement.to_owned(),
        }],
        hunk: String::new(),
        safe: true,
        isolation: None,
    }
}

#[test]
fn keeps_only_the_fixes_that_neither_overlap_nor_touch() {
    let first_word = edit(1, 1, 4, "");
    let overlapping = edit(1, 3, 6, "x");
    let same_start = edit(1, 1, 1, "y");
    let touching = edit(1, 4, 5, "z");
    let other_line = edit(2, 1, 1, "final ");
    let fixes = [first_word, overlapping, same_start, touching, other_line];
    let chosen = compatible(&fixes);
    assert_eq!(chosen.len(), 2);
    let fixed_text = apply(&edits_of(&chosen), "abcde\nint a;\n").expect("caen en el texto");
    assert_eq!(fixed_text, "de\nfinal int a;\n");
}

#[test]
fn writes_one_hunk_per_change_with_the_numbers_of_both_sides() {
    let old = "import a.B;\nimport c.D;\nclass A {\r\n  void m(int v) {}\n}\n";
    let new = "import c.D;\nclass A {\r\n  void m(final int v) {}\n}\n// fin\n";
    assert_eq!(
        file_patch("src/A.java", old, new).as_deref(),
        Some(
            "--- a/src/A.java\n+++ b/src/A.java\n@@ -1 +0,0 @@\n-import a.B;\n@@ -4 +3 @@\n-  void m(int v) {}\n+  void m(final int v) {}\n@@ -5,0 +5 @@\n+// fin\n"
        )
    );
    assert_eq!(file_patch("a", old, old), None);
}

#[test]
fn falls_back_to_one_hunk_when_everything_changes() {
    let old: String = (0..1_200)
        .map(|line| ["a", &line.to_string(), "\n"].concat())
        .collect();
    let new: String = (0..1_200)
        .map(|line| ["b", &line.to_string(), "\n"].concat())
        .collect();
    let patch = file_patch("big", &old, &new).expect("cambia");
    assert!(patch.contains("@@ -1,1200 +1,1200 @@"));
    assert_eq!(patch.matches("@@ ").count(), 1);
}

#[test]
fn the_patch_keeps_carriage_returns_and_marks_a_missing_final_newline() {
    let patch = file_patch("A.java", "import a.B;\r\nclass A {}\r\n", "class A {}\r\n").expect("cambió");
    assert_eq!(
        patch,
        "--- a/A.java\n+++ b/A.java\n@@ -1 +0,0 @@\n-import a.B;\r\n"
    );
    let unterminated = file_patch("A.java", "class A {}", "final class A {}").expect("cambió");
    assert_eq!(
        unterminated,
        "--- a/A.java\n+++ b/A.java\n@@ -1 +1 @@\n-class A {}\n\\ No newline at end of file\n+final class A {}\n\\ No newline at end of file\n"
    );
}
