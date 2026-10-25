use tree_sitter::Parser;

use super::{BIT_OR, unparse};
use crate::language::Language;

const LOWEST: u8 = 0;

fn rendered(source: &str, field: &str, level: u8) -> String {
    let mut parser = Parser::new();
    parser
        .set_language(&Language::Python.grammar().expect("tiene gramática"))
        .expect("la gramática carga");
    let tree = parser.parse(source, None).expect("se parsea");
    let assignment = tree
        .root_node()
        .named_child(0)
        .and_then(|statement| statement.named_child(0))
        .expect("hay una asignación");
    let node = assignment.child_by_field_name(field).expect("tiene el campo");
    unparse(node, level, source.as_bytes())
}

fn value(expression: &str) -> String {
    rendered(&format!("x = {expression}\n"), "right", LOWEST)
}

fn annotation(expression: &str) -> String {
    rendered(&format!("x: {expression}\n"), "type", LOWEST)
}

#[test]
fn prints_operators_with_the_parentheses_that_precedence_needs() {
    let cases = [
        ("a  and  b and c", "a and b and c"),
        ("(a or b) and c", "(a or b) and c"),
        ("not(a)", "not a"),
        ("not (a or b)", "not (a or b)"),
        ("(a+b)*c", "(a + b) * c"),
        ("a**(b**c)", "a ** b ** c"),
        ("(a**b)**c", "(a ** b) ** c"),
        ("-(a+b)", "-(a + b)"),
        ("a<b  not in  c", "a < b not in c"),
        ("a  is  not b", "a is not b"),
        ("(a if b else c)", "a if b else c"),
        ("(y:=1)", "y := 1"),
        ("lambda  a, b :a+b", "lambda a, b: a + b"),
        ("lambda:0", "lambda: 0"),
        ("1 .real", "(1).real"),
        ("a . b", "a.b"),
        ("'a'  'b'", "'a' 'b'"),
    ];
    for (source, expected) in cases {
        assert_eq!(value(source), expected, "{source}");
    }
}

#[test]
fn prints_calls_subscripts_and_containers() {
    let cases = [
        ("f( a ,b=1, *c, **d )", "f(a, b=1, *c, **d)"),
        ("f(i for i in y)", "f(i for i in y)"),
        ("a[ b ]", "a[b]"),
        ("a[b,c]", "a[b, c]"),
        ("a[1:2:3]", "a[1:2:3]"),
        ("a[:]", "a[:]"),
        ("[ 1,2 ]", "[1, 2]"),
        ("{ 1,2 }", "{1, 2}"),
        ("()", "()"),
        ("(1,)", "1,"),
        ("(1, 2)", "1, 2"),
        ("{'a':1, **b}", "{'a': 1, **b}"),
        ("[i  for i in y if i]", "[i for i in y if i]"),
        ("{i for i in y}", "{i for i in y}"),
        ("{k:v for k, v in y}", "{k: v for k, v in y}"),
        ("(i async for i in y)", "(i async for i in y)"),
    ];
    for (source, expected) in cases {
        assert_eq!(value(source), expected, "{source}");
    }
}

#[test]
fn prints_await_and_yield_inside_functions() {
    let source = "async def f():\n    x = await (g())\n    y = yield  1\n";
    let mut parser = Parser::new();
    parser
        .set_language(&Language::Python.grammar().expect("tiene gramática"))
        .expect("la gramática carga");
    let tree = parser.parse(source, None).expect("se parsea");
    let body = tree
        .root_node()
        .named_child(0)
        .and_then(|function| function.child_by_field_name("body"))
        .expect("tiene cuerpo");
    let values: Vec<String> = (0..2)
        .filter_map(|index| body.named_child(index))
        .filter_map(|statement| statement.named_child(0))
        .filter_map(|assignment| assignment.child_by_field_name("right"))
        .map(|node| unparse(node, LOWEST, source.as_bytes()))
        .collect();
    assert_eq!(values, ["await g()", "yield 1"]);
}

#[test]
fn prints_annotations_like_ruff() {
    let cases = [
        ("List[ int ]", "List[int]"),
        ("Dict[str,int]", "Dict[str, int]"),
        ("int|str", "int | str"),
        ("typing . Any", "typing.Any"),
    ];
    for (source, expected) in cases {
        assert_eq!(annotation(source), expected, "{source}");
    }
    assert_eq!(rendered("x: int|str\n", "type", BIT_OR + 1), "(int | str)");
}
