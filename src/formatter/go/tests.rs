use super::format;
use crate::formatter::Unparsable;

const SYNTAX_SOURCE: &str = include_str!("fixtures/syntax.go");
const SYNTAX_EXPECTED: &str = include_str!("fixtures/syntax.gofmt");
const COMMENTS_SOURCE: &str = include_str!("fixtures/comments.go");
const COMMENTS_EXPECTED: &str = include_str!("fixtures/comments.gofmt");

fn assert_formats_like_gofmt(source: &str, expected: &str) {
    assert_eq!(format(source).as_deref(), Ok(expected));
    assert_eq!(
        format(expected).as_deref(),
        Ok(expected),
        "formatear dos veces cambia"
    );
}

#[test]
fn reindents_a_small_file() {
    let source = "package main\nimport \"fmt\"\nfunc main(){\nfmt.Println(1+2*3)\n}\n";
    let expected = "package main\n\nimport \"fmt\"\n\nfunc main() {\n\tfmt.Println(1 + 2*3)\n}\n";
    assert_formats_like_gofmt(source, expected);
}

#[test]
fn formats_the_syntax_fixture_like_gofmt() {
    assert_formats_like_gofmt(SYNTAX_SOURCE, SYNTAX_EXPECTED);
}

#[test]
fn formats_the_comments_fixture_like_gofmt() {
    assert_formats_like_gofmt(COMMENTS_SOURCE, COMMENTS_EXPECTED);
}

#[test]
fn converts_line_endings_to_newlines() {
    let source = "package main\r\n\r\nvar x = `a\r\nb`\r\n// fin\r\n";
    assert_formats_like_gofmt(source, "package main\n\nvar x = `a\nb`\n\n// fin\n");
}

#[test]
fn reads_new_with_an_expression_from_go_1_26() {
    let source = "package main\nvar p=new(uint64(1))\nvar q = new(int)\n";
    assert_formats_like_gofmt(
        source,
        "package main\n\nvar p = new(uint64(1))\nvar q = new(int)\n",
    );
}

#[test]
#[ignore = "depuración local"]
fn debug_files() {
    let list = std::env::var("KERBO_FMT_LIST")
        .ok()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .unwrap_or_default();
    for path in list.split(':').filter(|path| !path.is_empty()) {
        let source = std::fs::read_to_string(path).unwrap();
        eprintln!("FILE {path}");
        let _ = format(&source);
    }
}

#[test]
fn leaves_files_with_syntax_errors_untouched() {
    assert_eq!(format("package main\nfunc {\n"), Err(Unparsable));
    assert_eq!(format("x := 1\n"), Err(Unparsable));
}
