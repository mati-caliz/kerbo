use tree_sitter::Parser;

use crate::analysis::analyze;
use crate::language::Language;
use crate::limits::Limits;

fn python_complexities(source: &str) -> Vec<(String, Option<usize>, Option<usize>)> {
    let mut parser = Parser::new();
    parser
        .set_language(&Language::Python.grammar())
        .expect("la gramática carga");
    analyze(
        &mut parser,
        Language::Python,
        source.as_bytes(),
        Limits::default(),
    )
    .expect("el archivo se parsea")
    .functions
    .into_iter()
    .map(|function| (function.name, function.cyclomatic, function.parameters))
    .collect()
}

#[test]
fn only_an_irrefutable_last_case_without_guard_counts_as_else() {
    let source = "def capture(x):
    match x:
        case 1:
            pass
        case other:
            pass

def alias(x):
    match x:
        case 1:
            pass
        case _ as unknown:
            pass

def guarded(x):
    match x:
        case 1:
            pass
        case _ if x:
            pass

def alternative(x):
    match x:
        case 1:
            pass
        case 2 | _:
            pass

def dotted(x):
    match x:
        case 1:
            pass
        case a.b:
            pass
";
    let complexities: Vec<(String, Option<usize>)> = python_complexities(source)
        .into_iter()
        .map(|(name, cyclomatic, _)| (name, cyclomatic))
        .collect();
    let expected = [
        ("capture", 2),
        ("alias", 2),
        ("guarded", 3),
        ("alternative", 2),
        ("dotted", 3),
    ];
    assert_eq!(
        complexities,
        expected
            .iter()
            .map(|&(name, value)| (name.to_owned(), Some(value)))
            .collect::<Vec<_>>()
    );
}

#[test]
fn overrides_and_overloads_skip_the_parameter_count_like_ruff() {
    let source = "import typing

class Child:
    @typing.override
    def message(self, msg, end):
        pass

    def other(self, msg, end):
        pass
";
    let counted: Vec<(String, Option<usize>)> = python_complexities(source)
        .into_iter()
        .map(|(name, _, parameters)| (name, parameters))
        .collect();
    assert_eq!(
        counted,
        vec![("message".to_owned(), None), ("other".to_owned(), Some(2))]
    );
}
