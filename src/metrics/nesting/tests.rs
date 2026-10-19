use tree_sitter::Parser;

use crate::analysis::analyze;
use crate::language::Language;

fn nesting_of(language: Language, source: &str, limit: usize) -> Vec<(usize, usize)> {
    let mut parser = Parser::new();
    parser
        .set_language(&language.grammar())
        .expect("la gramática carga");
    analyze(&mut parser, language, source.as_bytes(), Some(limit))
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
