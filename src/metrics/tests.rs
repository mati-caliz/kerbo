use tree_sitter::Parser;

use super::FunctionMetrics;
use crate::analysis::{FileAnalysis, analyze};
use crate::language::Language;

fn analyze_source(language: Language, source: &str) -> FileAnalysis {
    let mut parser = Parser::new();
    parser
        .set_language(&language.grammar())
        .expect("la gramática carga");
    analyze(&mut parser, language, source.as_bytes()).expect("el archivo se parsea")
}

fn function_named<'analysis>(analysis: &'analysis FileAnalysis, name: &str) -> &'analysis FunctionMetrics {
    analysis
        .functions
        .iter()
        .find(|function| function.name == name)
        .expect("la función existe")
}

const GO_SOURCE: &str = r#"package sample

func (r *T) Decide(a, b int, c string, d ...int) int {
	if a > 0 && b > 0 || c == "" {
		return 1
	} else if a < 0 {
		return 2
	} else {
		return 3
	}
outer:
	for i := range d {
		switch i {
		case 1:
			continue outer
		default:
			break
		}
	}
	square := func(x int) int { return x * x }
	return square(1)
}
"#;

#[test]
fn measures_go_like_cyclop_gocognit_funlen_and_revive() {
    let analysis = analyze_source(Language::Go, GO_SOURCE);
    let decide = function_named(&analysis, "Decide");
    assert_eq!(
        analysis.functions.len(),
        1,
        "func_literal se mide dentro de su función"
    );
    assert_eq!(decide.cyclomatic, 7);
    assert_eq!(decide.cognitive, 9);
    assert_eq!(decide.parameters, Some(4));
    assert_eq!(decide.lines, 18);
    assert_eq!(analysis.file_lines, 22);
}

const JAVA_SOURCE: &str = "class Sample {
  Sample(int value) {}

  @Override
  public boolean equals(Object a, Object b, Object c, Object d, Object e, Object f) { return true; }

  int decide(int a, String... rest) {
    // comentario

    if (a > 0 && a < 3 || a == 9) { return 1; } else if (a < 0) { return 2; } else { return 3; }
    outer: for (int i : new int[] {1}) { switch (i) { case 1: break outer; default: break; } }
    try { } catch (Exception e) { }
    Runnable r = () -> {};
    return a > 1 ? 1 : 0;
  }
}
";

#[test]
fn measures_java_like_checkstyle_and_pmd() {
    let analysis = analyze_source(Language::Java, JAVA_SOURCE);
    let decide = function_named(&analysis, "decide");
    assert_eq!(decide.cyclomatic, 9);
    assert_eq!(decide.cognitive, 11);
    assert_eq!(decide.parameters, Some(2));
    assert_eq!(decide.lines, 7);
    assert_eq!(
        function_named(&analysis, "Sample").parameters,
        None,
        "los constructores no cuentan"
    );
}

const TYPESCRIPT_SOURCE: &str = "class Sample {
  constructor(private a: number) {}
  m(x = 1, y?: string, ...z: number[]) { return x ?? 1; }
}

// comentario
function decide(a: number, b: string): number {
  /* comentario */

  if (((a && b) || a) ?? 2) { return 1; } else if (a) { return 2; } else { a ||= 3; }
  outer: for (const i of [1]) { switch (i) { case 1: continue outer; default: break; } }
  try {} catch (e) {}
  const read = (q) => q?.x;
  const single = p => p;
  return a ? 1 : 0;
}
";

#[test]
fn measures_typescript_like_eslint_and_sonarjs() {
    let analysis = analyze_source(Language::TypeScript, TYPESCRIPT_SOURCE);
    let decide = function_named(&analysis, "decide");
    assert_eq!(analysis.functions.len(), 5);
    assert_eq!(decide.cyclomatic, 10);
    assert_eq!(decide.cognitive, 11, "sonarjs no cuenta ??");
    assert_eq!(decide.parameters, Some(2));
    assert_eq!(decide.lines, 8);
    assert_eq!(function_named(&analysis, "m").parameters, Some(3));
    assert_eq!(function_named(&analysis, "constructor").parameters, Some(1));
    assert_eq!(function_named(&analysis, "read").parameters, Some(1));
    assert_eq!(function_named(&analysis, "single").parameters, Some(1));
    assert_eq!(analysis.file_lines, 12);
}

const PYTHON_SOURCE: &str = "def decide(self, a, b: int, c=1, *args, d: int = 2, **kw):
    # comentario
    if a and b or c:
        return 1
    elif a:
        pass
    else:
        x = 1 if a else 2
    try:
        pass
    except ValueError:
        pass
    while a:
        break
    return lambda y: y
";

#[test]
fn measures_python_like_mccabe_and_ruff() {
    let analysis = analyze_source(Language::Python, PYTHON_SOURCE);
    let decide = function_named(&analysis, "decide");
    assert_eq!(decide.cyclomatic, 5);
    assert_eq!(decide.cognitive, 9);
    assert_eq!(decide.parameters, Some(4), "self y cls no cuentan, como en ruff");
}

#[test]
fn nested_functions_are_measured_on_their_own() {
    let analysis = analyze_source(
        Language::JavaScript,
        "function outer() {\n  if (a) {}\n  function inner() { if (b) {} if (c) {} }\n}\n",
    );
    assert_eq!(function_named(&analysis, "outer").cyclomatic, 2);
    assert_eq!(function_named(&analysis, "inner").cyclomatic, 3);
}

#[test]
fn empty_file_has_no_lines() {
    let analysis = analyze_source(Language::Go, "");
    assert_eq!(analysis.file_lines, 0);
    assert!(analysis.functions.is_empty());
}
