use std::path::PathBuf;

use tree_sitter::Parser;

use super::{DuplicateUnits, PooledFile, duplicate_units, find_duplicates, image_hash};
use crate::language::Language;
use crate::profile::DuplicateStyle;
use crate::rules::{FileIdentity, RuleId};

const SMALL_TILE_LIMIT: usize = 9;

fn units_of(language: Language, style: DuplicateStyle, source: &str) -> DuplicateUnits {
    let mut parser = Parser::new();
    parser
        .set_language(&language.grammar())
        .expect("la gramática carga");
    let tree = parser.parse(source, None).expect("el archivo se parsea");
    duplicate_units(tree.root_node(), source.as_bytes(), style).expect("el lenguaje se mide")
}

fn tokens_of(language: Language, style: DuplicateStyle, source: &str) -> Vec<super::CpdToken> {
    match units_of(language, style, source) {
        DuplicateUnits::Tokens(tokens) => tokens,
        DuplicateUnits::Syntax(_) => Vec::new(),
    }
}

fn images(language: Language, style: DuplicateStyle, source: &str) -> Vec<u64> {
    tokens_of(language, style, source)
        .into_iter()
        .map(|token| token.image)
        .collect()
}

fn hashed(expected: &[&str]) -> Vec<u64> {
    expected.iter().map(|image| image_hash(image)).collect()
}

fn pooled(name: &str, source: &str, limit: usize) -> PooledFile {
    PooledFile {
        identity: FileIdentity {
            display: name.to_owned(),
            relative: name.to_owned(),
            absolute: PathBuf::from("/proyecto").join(name),
        },
        limit,
        units: units_of(Language::Java, DuplicateStyle::CpdJava, source),
    }
}

const REPEATED_METHOD: &str = "class A {
    int total(int[] values) {
        int sum = 0;
        for (int value : values) {
            sum += value;
        }
        return sum;
    }
}
";

#[test]
fn java_tokens_drop_imports_semicolons_and_annotations_like_cpd() {
    let source = "package a.b;\nimport java.util.List;\n@Ann(x = 1)\npublic class A { @Override int x; }\n";
    assert_eq!(
        images(Language::Java, DuplicateStyle::CpdJava, source),
        hashed(&["public", "class", "A", "{", "int", "x", "}"])
    );
}

#[test]
fn java_tokens_split_shifts_and_decode_unicode_escapes() {
    let source = "class A { List<List<String>> a = \"\\u0041\"; int b = 8 >> 1; }";
    assert_eq!(
        images(Language::Java, DuplicateStyle::CpdJava, source),
        hashed(&[
            "class", "A", "{", "List", "<", "List", "<", "String", ">", ">", "a", "=", "\"A\"", "int", "b",
            "=", "8", ">", ">", "1", "}",
        ])
    );
}

#[test]
fn cpd_off_comments_and_cpd_start_annotations_hide_code() {
    let source = "class A {\n// CPD-OFF\nint hidden;\n// CPD-ON\nint shown;\n\
                  @SuppressWarnings(\"CPD-START\") void a() {} @SuppressWarnings(\"CPD-END\") void c() {} }";
    assert_eq!(
        images(Language::Java, DuplicateStyle::CpdJava, source),
        hashed(&[
            "class", "A", "{", "int", "shown", "void", "c", "(", ")", "{", "}", "}"
        ])
    );
}

#[test]
fn python_lexer_follows_the_cpd_grammar() {
    let source = "x = 1_000 + 0x1F\ns = 'a\\\nb' + rb'z'\ndef f() -> str: ...  # CPD-ON\n";
    assert_eq!(
        images(Language::Python, DuplicateStyle::CpdPython, source),
        hashed(&[
            "x", "=", "1", "_000", "+", "0x1F", "s", "=", "'ab'", "+", "rb", "'z'", "def", "f", "(", ")",
            "-", ">", "str", ":", ".", ".", ".",
        ])
    );
}

#[test]
fn reports_every_copy_of_a_block_longer_than_the_limit() {
    let diagnostics = find_duplicates(vec![
        pooled("B.java", REPEATED_METHOD, SMALL_TILE_LIMIT),
        pooled("A.java", REPEATED_METHOD, SMALL_TILE_LIMIT),
    ]);
    let found: Vec<(&str, usize, usize)> = diagnostics
        .iter()
        .map(|diagnostic| (diagnostic.path.as_str(), diagnostic.line, diagnostic.value))
        .collect();
    assert_eq!(found, vec![("A.java", 1, 32), ("B.java", 1, 32)]);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.rule == RuleId::DuplicateCode)
    );
    assert!(diagnostics[0].message.contains("B.java:1"));
}

#[test]
fn a_block_at_the_limit_is_not_reported() {
    let tokens = tokens_of(Language::Java, DuplicateStyle::CpdJava, REPEATED_METHOD).len();
    assert!(
        find_duplicates(vec![
            pooled("A.java", REPEATED_METHOD, tokens),
            pooled("B.java", REPEATED_METHOD, tokens),
        ])
        .is_empty()
    );
}

#[test]
fn self_repeating_sequences_inside_one_file_report_once() {
    let source = "class A { void a() { x(); x(); x(); x(); x(); x(); x(); x(); x(); x(); x(); x(); } }";
    let diagnostics = find_duplicates(vec![pooled("A.java", source, 3)]);
    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0].value, diagnostics[1].value);
}

const GO_HANDLER: &str = "package p

func handle(values []int) int {
	total := 0
	for _, value := range values {
		if value > 0 {
			total += value
		}
	}
	return total
}
";

fn go_pooled(path: &str, source: &str, limit: usize) -> PooledFile {
    PooledFile {
        identity: FileIdentity {
            display: path.to_owned(),
            relative: path.to_owned(),
            absolute: PathBuf::from("/proyecto").join(path),
        },
        limit,
        units: units_of(Language::Go, DuplicateStyle::Dupl, source),
    }
}

fn go_kinds(source: &str) -> Vec<(i32, usize)> {
    match units_of(Language::Go, DuplicateStyle::Dupl, source) {
        DuplicateUnits::Syntax(nodes) => nodes.into_iter().map(|node| (node.kind, node.owns)).collect(),
        DuplicateUnits::Tokens(_) => Vec::new(),
    }
}

#[test]
fn go_nodes_follow_the_shape_and_order_of_go_ast() {
    use super::go_kinds::{ASSIGN_STMT, BLOCK_STMT, FIELD_LIST, FILE, FUNC_DECL, FUNC_TYPE, IDENT};
    assert_eq!(
        go_kinds("package p\n\nimport \"fmt\"\n\nfunc f() { a = b }\n"),
        vec![
            (FILE, 8),
            (FUNC_DECL, 7),
            (IDENT, 0),
            (FUNC_TYPE, 1),
            (FIELD_LIST, 0),
            (BLOCK_STMT, 3),
            (ASSIGN_STMT, 2),
            (IDENT, 0),
            (IDENT, 0),
        ],
        "sin el import, y la asignación con el lado derecho primero"
    );
}

#[test]
fn go_files_that_do_not_parse_are_left_out() {
    let mut parser = Parser::new();
    parser
        .set_language(&Language::Go.grammar())
        .expect("la gramática carga");
    let source = "package p\nfunc f( {";
    let tree = parser.parse(source, None).expect("el archivo se parsea");
    assert!(duplicate_units(tree.root_node(), source.as_bytes(), DuplicateStyle::Dupl).is_none());
}

#[test]
fn go_reports_each_copy_of_a_repeated_function_with_its_lines() {
    let diagnostics = find_duplicates(vec![
        go_pooled("p/a.go", GO_HANDLER, SMALL_TILE_LIMIT),
        go_pooled("p/b.go", GO_HANDLER, SMALL_TILE_LIMIT),
    ]);
    let found: Vec<(&str, usize, usize)> = diagnostics
        .iter()
        .map(|diagnostic| (diagnostic.path.as_str(), diagnostic.line, diagnostic.end_line))
        .collect();
    assert_eq!(
        found,
        vec![("p/a.go", 1, 11), ("p/b.go", 1, 11)],
        "dos archivos iguales: el bloque es el archivo entero, desde `package`"
    );
    assert!(diagnostics[0].message.contains("nodos"));
}

#[test]
fn go_files_are_pooled_by_package_directory() {
    let go_file = go_pooled("p/a.go", GO_HANDLER, SMALL_TILE_LIMIT);
    assert_eq!(go_file.package_scope(), Some(std::path::Path::new("/proyecto/p")));
    assert_eq!(
        pooled("A.java", REPEATED_METHOD, SMALL_TILE_LIMIT).package_scope(),
        None
    );
}

const GO_EVERY_CONSTRUCT: &str = r#"package p

import (
	"fmt"
	"io"
)

type (
	Number interface{ ~int | ~float64 }
	Pair[K comparable, V any] struct {
		Key   K `json:"key"`
		Value V
		*io.PipeReader
		fmt.Stringer
	}
	Reader interface {
		io.Reader
		Read(buffer []byte) (count int, err error)
		Close() error
	}
	Alias = map[string][]chan<- int
	Handler func(int, ...string) (bool, error)
)

const (
	first, second = iota, 2.5i
	third
)

var table = [...]struct{ name string }{{"a"}, {name: "b"}}

func Sum[T Number](values ...T) (total T) {
	for _, value := range values {
		total += value
	}
	return
}

func (pair *Pair[K, V]) Swap(other Pair[K, V]) Pair[K, V] {
	defer func() { _ = recover() }()
	go fmt.Println(pair.Key)
	channel := make(chan int, 1)
	channel <- 1
	select {
	case received, ok := <-channel:
		_, _ = received, ok
	case <-channel:
	case channel <- 2:
	default:
	}
	switch kind := any(other.Value).(type) {
	case int, string:
		fmt.Println(kind)
	case nil:
	default:
		fallthrough_free(kind)
	}
	switch value := 3; {
	case value > 2 && !false:
		value--
		fallthrough
	default:
		value++
	}
	if result, err := fmt.Println(); err != nil {
		goto done
	} else if result > 0 {
		var slice = []int{1, 2, 3}[1:2:3]
		_ = slice[0]
	} else {
		_ = &table
	}
loop:
	for index := 0; index < 3; index++ {
		for {
			break loop
		}
		continue loop
	}
	for true {
		break
	}
	_ = Sum[int](1, 2)
	_ = Sum[float64]
	_ = []byte("x")
	_ = (*Pair[K, V])(nil)
	_ = map[string]int{"a": 1}
	_ = func(x int) int { return -x * 'c' }
	_ = `raw` + "text"
done:
	;
	return *pair
}

func fallthrough_free(any) {}
"#;

#[test]
fn every_go_construct_maps_to_a_go_ast_node() {
    let kinds = go_kinds(GO_EVERY_CONSTRUCT);
    assert!(kinds.len() > 300, "{} nodos", kinds.len());
    assert!(
        kinds.iter().all(|&(kind, _)| kind != 0),
        "ningún nodo queda sin traducir"
    );
}

#[test]
fn type_switches_and_selects_take_the_shape_of_go_ast() {
    use super::go_kinds::{
        ASSIGN_STMT, BLOCK_STMT, CALL_EXPR, CASE_CLAUSE, COMM_CLAUSE, EXPR_STMT, IDENT, SELECT_STMT,
        TYPE_ASSERT_EXPR, TYPE_SWITCH_STMT, UNARY_EXPR,
    };
    let body = |source: &str| -> Vec<i32> {
        let kinds = go_kinds(&format!("package p\n\nfunc f() {{\n{source}\n}}\n"));
        kinds.into_iter().skip(6).map(|(kind, _)| kind).collect()
    };
    assert_eq!(
        body("switch x := y.(type) {\ncase int:\n}"),
        vec![
            TYPE_SWITCH_STMT,
            ASSIGN_STMT,
            TYPE_ASSERT_EXPR,
            IDENT,
            IDENT,
            BLOCK_STMT,
            CASE_CLAUSE,
            IDENT
        ]
    );
    assert_eq!(
        body("select {\ncase <-c:\n}"),
        vec![SELECT_STMT, BLOCK_STMT, COMM_CLAUSE, EXPR_STMT, UNARY_EXPR, IDENT]
    );
    assert_eq!(body("f[int](x)")[..2], [EXPR_STMT, CALL_EXPR]);
}
