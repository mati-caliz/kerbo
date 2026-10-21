use std::path::Path;
use std::sync::Arc;

use tree_sitter::Parser;

use super::definition::parse_rules;
use super::expressions::expression_length;
use super::fragments::type_length;
use super::matcher::{Binding, match_rule};
use super::tokens::{LeafKind, Origin, TokenTree, lex};
use super::{CrateMacros, MacroIndex, local_name};
use crate::analysis::{SourceUnit, analyze_unit};
use crate::language::Language;
use crate::limits::Limits;
use crate::metrics::FunctionMetrics;
use crate::test_support::TempTree;

fn origin(offset: usize) -> Origin {
    Origin {
        source: 0,
        offset,
        expanded: false,
    }
}

fn trees(text: &str) -> Vec<TokenTree> {
    lex(text, &origin).expect("se tokeniza")
}

fn texts(trees: &[TokenTree]) -> Vec<&str> {
    trees
        .iter()
        .filter_map(TokenTree::leaf)
        .map(|leaf| leaf.text.as_str())
        .collect()
}

#[test]
fn lexes_compound_punctuation_lifetimes_and_literals() {
    let lexed = trees("a => 'b 'c' r#\"x\"# b'y' 1.5e3 x.0.1 ..= :: /* nota */ // fin");
    assert_eq!(
        texts(&lexed),
        [
            "a", "=>", "'b", "'c'", "r#\"x\"#", "b'y'", "1.5e3", "x", ".", "0", ".", "1", "..=", "::"
        ]
    );
    assert_eq!(
        lexed.get(2).and_then(TokenTree::leaf).map(|leaf| leaf.kind),
        Some(LeafKind::Lifetime)
    );
    assert!(
        lex("(]", &origin).is_none(),
        "los delimitadores tienen que cerrar"
    );
}

#[test]
fn finds_where_an_expression_ends_like_the_rustc_parser() {
    let cases = [
        ("a + b, c", 3),
        ("0 1 i8::MIN", 1),
        ("i8::MIN i8::MAX", 3),
        ("|a, b| a + b, rest", 8),
        ("if a { b } else { c } => d", 5),
        ("Foo { a } + 1", 4),
        ("x.method::<u8, u16>()? as u32; next", 13),
        ("match x { _ => 1 } , y", 3),
        ("-value .. end", 4),
        ("return", 1),
    ];
    for (text, expected) in cases {
        assert_eq!(expression_length(&trees(text)), Some(expected), "{text}");
    }
    assert_eq!(
        expression_length(&trees("@inner")),
        None,
        "`@` no empieza una expresión"
    );
    assert_eq!(type_length(&trees("HashMap<K, V> = x")), Some(6));
}

#[test]
fn matches_repetitions_with_separators_and_backtracks_on_their_count() {
    let definition = trees("{ ($($name:ident),+ ; $last:tt) => {}; ($($rest:tt)*) => {} }");
    let rules = parse_rules(
        &definition
            .first()
            .and_then(TokenTree::group)
            .expect("hay un grupo")
            .trees,
    )
    .expect("las reglas se leen");
    let first = rules.first().expect("hay una primera regla");
    let bindings = match_rule(first, &trees("a, b, c ; d")).expect("coincide");
    let names = match bindings.get("name") {
        Some(Binding::Repeated(names)) => names.len(),
        _ => 0,
    };
    assert_eq!(names, 3);
    assert!(match_rule(first, &trees("a b")).is_none());
    assert!(
        rules
            .get(1)
            .and_then(|rule| match_rule(rule, &trees("a b")))
            .is_some()
    );
}

#[test]
fn only_paths_inside_the_crate_name_a_local_macro() {
    assert_eq!(local_name("check"), Some("check"));
    assert_eq!(local_name("crate :: macros :: check"), Some("check"));
    assert_eq!(local_name("$crate::check"), Some("check"));
    assert_eq!(local_name("log::debug"), None);
}

const LIBRARY: &str = "#[macro_use]
mod macros;
#[macro_use]
mod other;

pub fn from_other_module(a: u8) -> u8 {
    shared_if!(a) + defined_elsewhere!(a)
}

pub fn from_exported(a: u8) -> u8 {
    exported_match!(a) + private_if!(a)
}
";

const EXPORTED: &str = "#[macro_export]
#[doc(hidden)] // una nota entre el atributo y la macro
macro_rules! exported_match {
    ($v:expr) => {
        match $v {
            0 => 1,
            _ => 2,
        }
    };
}

macro_rules! private_if {
    ($v:expr) => {
        if $v > 0 { 1 } else { 2 }
    };
}
";

const MACROS: &str = "macro_rules! shared_if {
    ($v:expr) => {
        if $v > 0 { 1 } else { 2 }
    };
}

macro_rules! shared_function {
    ($name:ident) => {
        pub fn $name(first: u8, second: u8, third: u8) -> u8 {
            let total = first + second;
            total + third
        }
    };
}
";

const OTHER: &str = "macro_rules! defined_elsewhere {
    ($v:expr) => {
        match $v {
            1 => 3,
            _ => 4,
        }
    };
}

shared_function!(made_in_other);
";

fn crate_tree() -> TempTree {
    let tree = TempTree::new("macro-crate");
    tree.write("Cargo.toml", "[package]\nname = \"sample\"\n");
    tree.write("src/lib.rs", LIBRARY);
    tree.write("src/macros.rs", MACROS);
    tree.write("src/other.rs", OTHER);
    tree.write("src/exported.rs", EXPORTED);
    tree
}

fn measure(path: &Path, source: &str, macros: Option<&Arc<CrateMacros>>) -> Vec<FunctionMetrics> {
    let mut parser = Parser::new();
    parser
        .set_language(&Language::Rust.grammar().expect("tiene gramática"))
        .expect("la gramática carga");
    let unit = SourceUnit {
        language: Language::Rust,
        source: source.as_bytes(),
        path: Some(path),
        crate_macros: macros,
    };
    analyze_unit(&mut parser, unit, Limits::default())
        .expect("el archivo se parsea")
        .functions
}

#[test]
fn reads_the_macros_of_the_whole_crate_before_measuring_a_file() {
    let tree = crate_tree();
    let index = MacroIndex::for_one_scan();
    let library = tree.path().join("src/lib.rs");
    let macros = index.crate_of(Language::Rust, &library);
    let calling = measure(&library, LIBRARY, macros.as_ref());
    let cognitive: Vec<Option<usize>> = calling.iter().map(|function| function.cognitive).collect();
    assert_eq!(
        cognitive,
        [Some(3), Some(2)],
        "se expanden las macros de los módulos con #[macro_use] y las exportadas, no las que quedan en su archivo"
    );
    let defining = measure(&tree.path().join("src/macros.rs"), MACROS, macros.as_ref());
    let built: Vec<(usize, Option<usize>, Option<usize>)> = defining
        .iter()
        .map(|function| (function.line, function.lines, function.parameters))
        .collect();
    assert_eq!(
        built,
        [(9, Some(2), None), (9, None, Some(3))],
        "la función que arma la macro se marca en la definición aunque la invocación esté en otro archivo"
    );
    let invoking = measure(&tree.path().join("src/other.rs"), OTHER, macros.as_ref());
    assert!(
        invoking.is_empty(),
        "en el archivo de la invocación no queda nada"
    );
    assert!(index.crate_of(Language::Python, &library).is_none());
}

#[test]
fn the_shared_index_notices_changed_files_and_prefers_the_text_in_the_editor() {
    let tree = crate_tree();
    let library = tree.path().join("src/lib.rs");
    let before = MacroIndex::shared().crate_of(Language::Rust, &library);
    assert_eq!(
        before
            .as_ref()
            .map(|macros| macros.definitions_named("added").len()),
        Some(0)
    );
    tree.write(
        "src/macros.rs",
        &format!("{MACROS}\nmacro_rules! added {{\n    () => {{}};\n}}\n"),
    );
    let after = MacroIndex::shared().crate_of(Language::Rust, &library);
    assert_eq!(
        after
            .as_ref()
            .map(|macros| macros.definitions_named("added").len()),
        Some(1)
    );
    let edited = LIBRARY.replace("shared_if!(a) + ", "");
    let measured = measure(&library, &edited, after.as_ref());
    assert_eq!(measured.first().and_then(|function| function.cognitive), Some(2));
}

#[test]
fn the_cache_context_only_changes_with_what_reaches_other_files() {
    let tree = crate_tree();
    tree.write(
        "src/plain.rs",
        "pub fn plain() -> u8 {
    1
}
",
    );
    let library = tree.path().join("src/lib.rs");
    let context = || {
        MacroIndex::for_one_scan()
            .crate_of(Language::Rust, &library)
            .map(|macros| macros.analysis_context(&library))
    };
    let original = context();
    tree.write(
        "src/plain.rs",
        "pub fn plain() -> u8 {
    2
}
",
    );
    assert_eq!(
        context(),
        original,
        "una función común no toca a los demás archivos"
    );
    tree.write(
        "src/macros.rs",
        &format!(
            "{MACROS}
macro_rules! added {{
    () => {{}};
}}
"
        ),
    );
    assert_ne!(context(), original, "una macro nueva sí");
    let with_macro = context();
    tree.write(
        "src/fresh.rs",
        "pub fn fresh() {}
",
    );
    assert_ne!(context(), with_macro, "un archivo nuevo también");
}

const HELPER: &str = "#[macro_export]
macro_rules! branch {
    ($flag:expr) => {
        if $flag { 1 } else { 0 }
    };
}

#[macro_export]
macro_rules! nested {
    () => {
        $crate::branch!(true)
    };
}

#[macro_export]
macro_rules! str {
    [[$data:literal]] => {
        $crate::branch!($data.is_empty())
    };
}

macro_rules! hidden {
    () => {
        if true { 1 } else { 0 }
    };
}
";

const APPLICATION: &str = "use helper::branch;
use helper::str;

pub fn imported(flag: bool) -> u8 {
    branch!(flag) + branch!(flag)
}

pub fn pathed() -> u8 {
    helper::nested!()
}

pub fn primitive_named() -> u8 {
    str![[\"x\"]]
}

pub fn not_exported() -> u8 {
    helper::hidden!()
}
";

#[test]
fn expands_the_exported_macros_of_dependencies() {
    let tree = TempTree::new("macro-dependencies");
    tree.write(
        "app/Cargo.toml",
        "[package]\nname = \"app\"\n\n[dependencies]\nhelper = { path = \"../helper\" }\n",
    );
    tree.write("app/src/lib.rs", APPLICATION);
    tree.write("helper/Cargo.toml", "[package]\nname = \"helper\"\n");
    tree.write("helper/src/lib.rs", HELPER);
    let library = tree.path().join("app/src/lib.rs");
    let macros = MacroIndex::for_one_scan().crate_of(Language::Rust, &library);
    let cognitive: Vec<Option<usize>> = measure(&library, APPLICATION, macros.as_ref())
        .iter()
        .map(|function| function.cognitive)
        .collect();
    assert_eq!(cognitive, [Some(3), Some(2), Some(2), Some(1)]);
}
