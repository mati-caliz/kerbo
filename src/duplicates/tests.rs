use std::path::PathBuf;

use tree_sitter::Parser;

use super::{PooledFile, cpd_tokens, find_duplicates, image_hash};
use crate::language::Language;
use crate::profile::DuplicateStyle;
use crate::rules::{FileIdentity, RuleId};

const SMALL_TILE_LIMIT: usize = 9;

fn tokens_of(language: Language, style: DuplicateStyle, source: &str) -> Vec<super::CpdToken> {
    let mut parser = Parser::new();
    parser
        .set_language(&language.grammar())
        .expect("la gramática carga");
    let tree = parser.parse(source, None).expect("el archivo se parsea");
    cpd_tokens(tree.root_node(), source.as_bytes(), style).expect("el lenguaje se mide")
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
        tokens: tokens_of(Language::Java, DuplicateStyle::CpdJava, source),
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
    let tokens = pooled("A.java", REPEATED_METHOD, SMALL_TILE_LIMIT).tokens.len();
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
