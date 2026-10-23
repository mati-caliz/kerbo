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

// Un grupo con `/* */` no se reacomoda, y las directivas van al final de la documentación separadas
// por un `//`, salvo que no haya texto.
#[test]
fn doc_comments_keep_block_comments_and_move_directives_last() {
    let source = "package p\n\n/* a */\n/* b */\nfunc f() {}\n\n/* c */\n// d\nfunc g() {}\n\n// e\n//go:noinline\nfunc h() {}\n\n//go:generate stringer\n// I is a number.\ntype I int\n\n//go:noinline\nfunc k() {}\n";
    let expected = "package p\n\n/* a */\n/* b */\nfunc f() {}\n\n/* c */\n// d\nfunc g() {}\n\n// e\n//\n//go:noinline\nfunc h() {}\n\n// I is a number.\n//\n//go:generate stringer\ntype I int\n\n//go:noinline\nfunc k() {}\n";
    assert_formats_like_gofmt(source, expected);
}

// `SortImports` junta las líneas en blanco entre el último import y el `)`.
#[test]
fn drops_the_blank_lines_before_the_closing_paren_of_the_imports() {
    let source = "package p\n\nimport (\n\t\"os\"\n\t_ \"unsafe\" // for linkname\n\n)\n\nimport (\n\t\"fmt\"\n\n\n)\n\nvar _ = os.Args\nvar _ = fmt.Sprint\n";
    let expected = "package p\n\nimport (\n\t\"os\"\n\t_ \"unsafe\" // for linkname\n)\n\nimport (\n\t\"fmt\"\n)\n\nvar _ = os.Args\nvar _ = fmt.Sprint\n";
    assert_formats_like_gofmt(source, expected);
}

// tree-sitter-go lee `if ok {...}` seguido de un bloque suelto como el literal `ok{...}` con el bloque
// de cuerpo; Go no. Mejor no tocar el archivo que romperlo.
#[test]
fn leaves_untouched_an_if_that_tree_sitter_reads_as_a_literal() {
    let source = "package main\n\nfunc f() {\n\tif !c.Ready {\n\t\tg(c)\n\t}\n\n\t{\n\t\th()\n\t}\n}\n";
    assert_eq!(format(source), Err(Unparsable));
    let literal = "package main\n\nfunc f() {\n\tif x == (T{}) {\n\t}\n\tfor _, v := range []T{{}} {\n\t\tg(v)\n\t}\n}\n";
    assert_eq!(format(literal).as_deref(), Ok(literal));
}

// `*(*struct{...})(p).f` y `chan<- chan T` tree-sitter-go los lee distinto que Go: formatearlos cambiaría el
// programa. Lo que sí es así (`(*p).f`, `<-chan <-chan T`) se formatea.
#[test]
fn leaves_untouched_what_tree_sitter_reads_with_another_meaning() {
    let deref =
        "package main\n\nfunc f() {\n\t*(*struct {\n\t\tp *byte\n\t})(unsafe.Pointer(&s)).p = 'H'\n}\n";
    assert_eq!(format(deref), Err(Unparsable));
    let send = "package main\n\nvar c chan<- chan string\n";
    assert_eq!(format(send), Err(Unparsable));
    let valid = "package main\n\nvar c <-chan <-chan string\nvar d chan<- <-chan int\n\nfunc f() {\n\t(*p).f = 1\n}\n";
    assert_eq!(format(valid).as_deref(), Ok(valid));
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
