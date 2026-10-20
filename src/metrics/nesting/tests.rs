use tree_sitter::Parser;

use crate::analysis::analyze;
use crate::language::Language;
use crate::limits::Limits;
use crate::rules::RuleId;

fn nesting_of(language: Language, source: &str, limit: usize) -> Vec<(usize, usize)> {
    let mut parser = Parser::new();
    parser
        .set_language(&language.grammar().expect("tiene gramática"))
        .expect("la gramática carga");
    analyze(
        &mut parser,
        language,
        source.as_bytes(),
        Limits::default().with_rule(RuleId::NestingDepth, Some(limit)),
    )
    .expect("el archivo se parsea")
    .nesting
    .into_iter()
    .map(|found| (found.line, found.depth))
    .collect()
}

#[test]
fn measures_nesting_like_eslint_max_depth() {
    let source = "if (a) {}\nfunction f() {\n  if (a) {\n    for (;;) {\n      while (b) {}\n    }\n  } else if (c) {\n    try {} catch {}\n  }\n  switch (d) {}\n  const g = () => { if (e) {} };\n}\n";
    assert_eq!(
        nesting_of(Language::TypeScript, source, 0),
        [(1, 1), (3, 1), (4, 2), (5, 3), (8, 2), (11, 1)],
        "el else if no suma al entrar pero resta al salir: el switch queda en 0 y no se reporta"
    );
    assert_eq!(nesting_of(Language::TypeScript, source, 2), [(5, 3)]);
}

#[test]
fn measures_nesting_like_revive_max_control_nesting() {
    let source = "package a\n\nfunc f() {\n\tif a {\n\t\tfor x := range xs {\n\t\t\tif b {\n\t\t\t}\n\t\t}\n\t} else if c {\n\t\tswitch d {\n\t\tcase 1:\n\t\t\tg()\n\t\t}\n\t}\n\th := func() {\n\t\tfor {\n\t\t}\n\t}\n}\n";
    assert_eq!(
        nesting_of(Language::Go, source, 1),
        [(6, 2), (9, 2)],
        "el range no suma, el else if sí, y la función anónima arranca de cero"
    );
    let nothing: Vec<(usize, usize)> = Vec::new();
    assert_eq!(nesting_of(Language::Go, source, 3), nothing);
    assert_eq!(
        nesting_of(Language::Java, "class A { void f() { if (a) { if (b) {} } } }", 0),
        nothing
    );
}

#[test]
fn nests_only_the_blocks_the_expansion_leaves_in_sight() {
    let lines: Vec<usize> = nesting_of(
        Language::Rust,
        "macro_rules! twice {
    ($b:block) => {
        $b;
        $b
    };
}

fn nested_args() {
    twice!({ { { let _ = 1; } } });
}

macro_rules! wrapped {
    ($e:expr) => {
        { { $e } }
    };
}

fn nested_wrapped() -> u8 {
    wrapped!({ { 1 } })
}

fn std_blocks(items: &[u8]) {
    assert!(items.iter().all(|item| { *item > 0 }));
    debug_assert!(items.iter().all(|item| { *item > 0 }));
    let _ = vec![{ { 1 } }];
}
",
        1,
    )
    .into_iter()
    .map(|(line, _)| line)
    .collect();
    assert_eq!(
        lines,
        [9, 23, 25],
        "un bloque de la expansión corta el recorrido: el de wrapped! y el if de debug_assert!; el \
         bloque repetido de twice! se marca una sola vez"
    );
}
