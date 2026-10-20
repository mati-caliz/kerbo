use tree_sitter::{Node, Parser, Tree};

use super::{Fix, TextEdit, TextPoint, apply};
use crate::language::Language;
use crate::syntax::descendants;

fn parse(source: &str) -> Tree {
    let mut parser = Parser::new();
    parser
        .set_language(&Language::Java.grammar())
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
