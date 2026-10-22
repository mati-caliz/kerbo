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
fn leaves_files_with_syntax_errors_untouched() {
    assert_eq!(format("package main\nfunc {\n"), Err(Unparsable));
    assert_eq!(format("x := 1\n"), Err(Unparsable));
}

// gofmt saca los paréntesis de una condición salvo que adentro haya un literal compuesto con nombre
// de tipo, aunque esté dentro de una función anónima.
const KEPT_PARENS: &str = "package main

func main() {
	if (func() bool {
		var a T
		var b, c = [...]int{1}, s[1:2:3]
		var d struct{ f [3]int }
		type L[P any] []P
		_ = G[int, string](*p, -q, x.(interface{ M() }), t+b[0])
		_ = func(n ...int) (r map[string]chan int) { return }
	next:
		for c.f, d = range m {
			break next
		}
		a.x++
		ch <- 1
		go f()
		defer f()
		for i := 0; i < 1; i++ {
		}
		for range xs {
		}
		switch {
		case true:
		}
		switch v := y.(type) {
		case int:
			_ = v
		}
		select {
		case <-ch:
		}
		{
		}
		if true {
		} else {
		}
		return a.x[0] == T{}.x && xs[1:2] != nil
	}()) {
	}
	if (x) {
	}
}
";

#[test]
fn keeps_the_parentheses_that_protect_a_composite_literal() {
    let formatted = format(KEPT_PARENS).expect("se formatea");
    assert!(formatted.contains("\tif (func() bool {\n"), "{formatted}");
    assert!(formatted.contains("\tif x {\n"), "{formatted}");
}
