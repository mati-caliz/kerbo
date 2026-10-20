use tree_sitter::Parser;

use crate::analysis::{FileAnalysis, analyze};
use crate::language::Language;
use crate::limits::Limits;
use crate::metrics::FunctionMetrics;
use crate::rules::RuleId;

pub(super) fn analyze_rust(source: &str, limits: Limits) -> FileAnalysis {
    let mut parser = Parser::new();
    parser
        .set_language(&Language::Rust.grammar().expect("tiene gramática"))
        .expect("la gramática carga");
    analyze(&mut parser, Language::Rust, source.as_bytes(), limits).expect("el archivo se parsea")
}

pub(super) fn functions(source: &str) -> Vec<FunctionMetrics> {
    analyze_rust(source, Limits::default()).functions
}

pub(super) fn named<'list>(functions: &'list [FunctionMetrics], name: &str) -> &'list FunctionMetrics {
    functions
        .iter()
        .find(|function| function.name == name)
        .expect("la función existe")
}

#[test]
fn counts_cognitive_complexity_over_the_desugared_code_like_clippy() {
    let measured = functions(
        "fn sample(items: &[u8]) -> Result<u8, Error> {
    for item in items {
        if *item > 0 {
            continue;
        }
    }
    while let Some(next) = queue.pop() {}
    let value = parse(items)?;
    match value {
        0 => {}
        big if big > 3 => {}
        _ => {}
    }
    assert!(value > 0);
    Ok(value)
}

fn optional(input: Option<u8>) -> Option<u8> {
    let value = input?;
    Some(value)
}
",
    );
    assert_eq!(
        named(&measured, "sample").cognitive,
        Some(7),
        "for, if, while, ?, match, guarda y assert! suman; el return del ? se descuenta entero con Result"
    );
    assert_eq!(
        named(&measured, "optional").cognitive,
        Some(2),
        "sin Result se descuenta la mitad de los returns"
    );
}

#[test]
fn a_return_right_after_another_is_not_discounted() {
    let measured = functions(
        "fn arms(flag: bool) {
    match flag {
        true => return,
        false => return,
    }
}

fn branches(flag: bool) {
    if flag {
        return;
    } else {
        return;
    }
}
",
    );
    assert_eq!(named(&measured, "arms").cognitive, Some(2));
    assert_eq!(
        named(&measured, "branches").cognitive,
        Some(1),
        "entre los dos returns está el bloque del else"
    );
}

#[test]
fn closures_are_measured_apart_also_inside_macros() {
    let measured = functions(
        "fn outer(values: &[bool]) {
    let pick = |value: u8| if value > 0 { 1 } else { 2 };
    assert!(values.iter().all(|value| if *value { true } else { false }));
}
",
    );
    assert_eq!(named(&measured, "outer").cognitive, Some(2));
    let closures: Vec<(usize, Option<usize>)> = measured
        .iter()
        .filter(|function| function.name != "outer")
        .map(|function| (function.line, function.cognitive))
        .collect();
    assert_eq!(closures, [(2, Some(2)), (3, Some(2))]);
}

#[test]
fn counts_lines_from_the_text_like_too_many_lines() {
    let body = "
    let first = 1; // comentario

    /* un bloque
       de comentario */
    let quoted = r\"
// esta línea es parte del string
\";
";
    let measured = functions(&format!("fn plain() {{{body}}}\n\nasync fn later() {{{body}}}\n"));
    assert_eq!(
        named(&measured, "plain").lines,
        Some(3),
        "la línea del string que empieza con // cuenta como comentario"
    );
    assert_eq!(
        named(&measured, "later").lines,
        Some(5),
        "en una función async las llaves también suman"
    );
}

#[test]
fn counts_arguments_like_too_many_arguments() {
    let measured = functions(
        "trait Shape {
    fn scale(&self, factor: f64);
}

impl Shape for Square {
    fn scale(&self, factor: f64) {}
}

impl Square {
    fn new(side: f64, color: Color) -> Self {}
    fn area(&self) -> f64 {}
}

extern \"C\" fn callback(data: *mut u8, length: usize) {}
",
    );
    let counted: Vec<(usize, Option<usize>)> = measured
        .iter()
        .map(|function| (function.line, function.parameters))
        .collect();
    assert_eq!(
        counted,
        [(2, Some(2)), (6, None), (10, Some(2)), (11, Some(1)), (14, None)],
        "el receptor cuenta; la implementación de un trait y otra ABI no se miran"
    );
}

#[test]
fn reports_the_first_block_nested_beyond_the_limit() {
    let analysis = analyze_rust(
        "impl Square {
    fn grow(&mut self) {
        if self.side > 0 {
            loop {}
        }
    }
}

mod inner {
    fn flat() {}
}
",
        Limits::default().with_rule(RuleId::NestingDepth, Some(2)),
    );
    let lines: Vec<(usize, usize)> = analysis
        .nesting
        .iter()
        .map(|finding| (finding.line, finding.depth))
        .collect();
    assert_eq!(
        lines,
        [(3, 3)],
        "impl, cuerpo y bloque del if; el loop de adentro ya no se mira, y el módulo suma uno solo"
    );
}

#[test]
fn resolves_results_without_types() {
    let measured = functions(
        "fn aliased(input: &str) -> ClapResult<u8> {
    let value = input.parse()?;
    Ok(value)
}

fn record(input: &str) -> SearchResult {
    let value = input.parse()?;
    SearchResult { value }
}

fn closures(items: &[&str]) {
    let parsed = items.iter().map(|item| {
        let value = item.parse::<u8>()?;
        Ok(value)
    });
    let first = items.first().and_then(|item| {
        let value = item.strip_prefix('-')?;
        value.parse().ok()
    });
}
",
    );
    assert_eq!(
        named(&measured, "aliased").cognitive,
        Some(1),
        "un alias genérico de Result"
    );
    assert_eq!(
        named(&measured, "record").cognitive,
        Some(2),
        "un struct que se llama *Result no lo es"
    );
    let closures: Vec<Option<usize>> = measured
        .iter()
        .filter(|function| function.name != "closures" && function.line > 10)
        .map(|function| function.cognitive)
        .collect();
    assert_eq!(
        closures,
        [Some(1), Some(2)],
        "termina en Ok(..): Result; si no, Option"
    );
}

#[test]
fn skips_unsafe_blocks_when_nesting() {
    let analysis = analyze_rust(
        "fn raw(bytes: &[u8]) -> u8 {
    unsafe {
        if bytes.is_empty() {
            loop {}
        }
    }
}
",
        Limits::default().with_rule(RuleId::NestingDepth, Some(1)),
    );
    assert_eq!(
        analysis.nesting.len(),
        0,
        "el span de un bloque unsafe arranca en `unsafe` y clippy no lo mira"
    );
}
