use tree_sitter::Parser;

use super::FunctionMetrics;
use crate::analysis::{FileAnalysis, analyze};
use crate::language::Language;

fn analyze_source(language: Language, source: &str) -> FileAnalysis {
    let mut parser = Parser::new();
    parser
        .set_language(&language.grammar())
        .expect("la gramática carga");
    analyze(&mut parser, language, source.as_bytes(), None).expect("el archivo se parsea")
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
    assert_eq!(decide.cyclomatic, Some(8), "cyclop cuenta también el default");
    assert_eq!(decide.cognitive, Some(9));
    assert_eq!(decide.parameters, Some(4));
    assert_eq!(decide.lines, Some(18));
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
    assert_eq!(decide.cyclomatic, Some(9));
    assert_eq!(decide.cognitive, Some(11));
    assert_eq!(decide.parameters, Some(2));
    assert_eq!(decide.lines, Some(7));
    assert_eq!(
        function_named(&analysis, "Sample").parameters,
        None,
        "los constructores no cuentan"
    );
    assert_eq!(
        function_named(&analysis, "equals").parameters,
        None,
        "ignoreOverriddenMethods"
    );
}

#[test]
fn measures_java_records_and_pmd_cognitive() {
    let record = analyze_source(
        Language::Java,
        "record R(int a) {\n  R {\n    a = a > 0 ? a : 0;\n  }\n}\n",
    );
    let compact = function_named(&record, "R");
    assert_eq!(
        compact.cyclomatic,
        Some(2),
        "Checkstyle 14 mide los constructores compactos"
    );
    assert_eq!(compact.cognitive, None, "PMD no los mide");
    let pmd = analyze_source(
        Language::Java,
        "class P {\n  int walk(int n) {\n    boolean ok = l.stream().anyMatch(x -> x == null || x > 1) && l.stream().anyMatch(y -> y || z);\n    if (n > 0) { return walk(n - 1); } else if (n < 0) { if (n < -5) { return 1; } }\n    return !ok ? 0 : 1;\n  }\n  abstract void hook();\n}\n",
    );
    assert_eq!(
        function_named(&pmd, "walk").cognitive,
        Some(8),
        "operador corrido entre lambdas, recursión y else if"
    );
    assert_eq!(
        function_named(&pmd, "hook").lines,
        None,
        "Checkstyle no mide métodos sin cuerpo"
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
    assert_eq!(decide.cyclomatic, Some(11), "ESLint cuenta ||=");
    assert_eq!(
        decide.cognitive,
        Some(10),
        "sonarjs sólo cuenta las secuencias de &&"
    );
    assert_eq!(decide.parameters, Some(2));
    assert_eq!(decide.lines, Some(8));
    assert_eq!(function_named(&analysis, "m").parameters, Some(3));
    assert_eq!(function_named(&analysis, "constructor").parameters, Some(1));
    assert_eq!(function_named(&analysis, "read").parameters, Some(1));
    assert_eq!(function_named(&analysis, "single").parameters, Some(1));
    assert_eq!(
        function_named(&analysis, "read").cyclomatic,
        Some(2),
        "ESLint cuenta ?."
    );
    assert_eq!(
        function_named(&analysis, "m").cyclomatic,
        Some(3),
        "ESLint cuenta los valores por defecto y ??"
    );
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
    assert_eq!(decide.cyclomatic, Some(5));
    assert_eq!(decide.cognitive, Some(9));
    assert_eq!(decide.parameters, Some(5), "fuera de una clase, self cuenta");
    let factory = analyze_source(
        Language::Python,
        "def factory(_unused, size):\n    def inner(x):\n        if x:\n            return 1\n    return inner\n",
    );
    let outer = function_named(&factory, "factory");
    assert_eq!(
        outer.cyclomatic,
        Some(3),
        "ruff suma la def anidada y sus decisiones"
    );
    assert_eq!(outer.parameters, Some(1), "ruff no cuenta los nombres dummy");
    let receivers = analyze_source(
        Language::Python,
        "class A:\n    @overload\n    def f(self, a): ...\n    @staticmethod\n    def g(self, b):\n        pass\n    def m(this, b):\n        pass\n\ndef h(self, *, c):\n    pass\n",
    );
    assert_eq!(
        function_named(&receivers, "f").parameters,
        None,
        "ruff no revisa los @overload"
    );
    assert_eq!(
        function_named(&receivers, "h").parameters,
        Some(2),
        "fuera de una clase, self cuenta"
    );
    assert_eq!(function_named(&factory, "inner").cyclomatic, Some(2));
    let branches = analyze_source(
        Language::Python,
        "def pick(x):\n    try:\n        pass\n    except E:\n        pass\n    else:\n        pass\n    for i in x:\n        pass\n    else:\n        pass\n    match x:\n        case 1:\n            pass\n        case _:\n            pass\n",
    );
    assert_eq!(
        function_named(&branches, "pick").cyclomatic,
        Some(5),
        "else de try sí, de for no; case _ no"
    );
}

const ESLINT_DETAILS_SOURCE: &str = "class Controller {
  @Get()
  @Auth()
  login(a: number) {
    return a;
  }
  handler = () => 1;
  mode = a ? 1 : 2;
  static {
    if (x) {}
  }
}
type Render = (props: { a: number }) => void;
declare function sign(a: number, b: string): void;
function leaky(a) {
  const pick = a ? () => 1 : null;
  if (a) {}
  notify?.(a);
  return pick;
}
function view() {
  const { size = 1 } = props;
  return <div>{a && b && <X />}{a && (b || c) && d}{a && (b ? <X /> : null)}</div>;
}
";

#[test]
fn measures_what_eslint_and_sonarjs_measure() {
    let analysis = analyze_source(Language::Tsx, ESLINT_DETAILS_SOURCE);
    assert_eq!(
        function_named(&analysis, "login").lines,
        Some(5),
        "los decoradores son parte del método"
    );
    let field_complexities: Vec<Option<usize>> = analysis
        .functions
        .iter()
        .filter(|function| function.lines.is_none() && function.parameters.is_none())
        .map(|function| function.cyclomatic)
        .collect();
    assert_eq!(
        field_complexities,
        [Some(1), Some(2), Some(2)],
        "campos con valor y bloque static"
    );
    assert_eq!(function_named(&analysis, "Render").parameters, Some(1));
    assert_eq!(function_named(&analysis, "Render").cyclomatic, None);
    assert_eq!(function_named(&analysis, "sign").parameters, Some(2));
    let template = analyze_source(
        Language::TypeScript,
        "function prompt() {\n  return `uno\n\n  dos`;\n}\n",
    );
    assert_eq!(
        function_named(&template, "prompt").lines,
        Some(4),
        "ESLint saltea los blancos de un template"
    );
    let iife = analyze_source(
        Language::TypeScript,
        "void (async () => {\n  await run();\n})();\nconst sql = () =>\n  q`\n  select 1\n  from t\n  `;\n",
    );
    let mut iife_lines = iife.functions.iter().map(|function| function.lines);
    assert_eq!(iife_lines.next(), Some(None), "ESLint saltea las IIFE");
    assert_eq!(
        iife_lines.next(),
        Some(Some(5)),
        "las líneas con texto de un template cuentan"
    );
    let optional = analyze_source(
        Language::JavaScript,
        "const pick = (b) => b.m(/x/)?.[1] ?? d?.();\n",
    );
    assert_eq!(
        function_named(&optional, "pick").cyclomatic,
        Some(4),
        "?. en la gramática de JavaScript"
    );
    let leaky = function_named(&analysis, "leaky");
    assert_eq!(
        leaky.cognitive,
        Some(3),
        "sonarjs deja +1 de anidamiento tras una flecha en un ternario"
    );
    assert_eq!(leaky.cyclomatic, Some(4), "ESLint cuenta el ?. de una llamada");
    let view = function_named(&analysis, "view");
    assert_eq!(view.cyclomatic, Some(9));
    assert_eq!(
        view.cognitive,
        Some(4),
        "el && que pinta JSX no suma; los demás, por secuencia"
    );
}

#[test]
fn nested_functions_are_measured_on_their_own() {
    let analysis = analyze_source(
        Language::JavaScript,
        "function outer() {\n  if (a) {}\n  function inner() { if (b) {} if (c) {} }\n}\n",
    );
    assert_eq!(function_named(&analysis, "outer").cyclomatic, Some(2));
    assert_eq!(function_named(&analysis, "inner").cyclomatic, Some(3));
}

#[test]
fn empty_file_has_no_lines() {
    let analysis = analyze_source(Language::Go, "");
    assert_eq!(analysis.file_lines, 0);
    assert!(analysis.functions.is_empty());
}

#[test]
fn counts_go_recursion_and_unnamed_parameters_like_gocognit_and_revive() {
    let analysis = analyze_source(
        Language::Go,
        "package a\n\nfunc walk(n int) int {\n\tif n > 0 {\n\t\treturn walk(n - 1)\n\t}\n\treturn 0\n}\n\nfunc ignore(*T, string) {}\n\nfunc (r R) walk(n int) int {\n\treturn walk(n)\n}\n",
    );
    let walk = analysis.functions.first().expect("hay funciones");
    assert_eq!(walk.cognitive, Some(2), "gocognit suma la llamada recursiva");
    assert_eq!(
        function_named(&analysis, "ignore").parameters,
        Some(0),
        "revive cuenta nombres"
    );
    assert_eq!(
        analysis.functions.last().and_then(|method| method.cognitive),
        Some(0),
        "un método no es recursión para gocognit"
    );
}
