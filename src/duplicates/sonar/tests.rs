use tree_sitter::Parser;

use super::{RepeatedKind, repeated_blocks};
use crate::language::Language;

const SONAR_MIN_LINES_LIMIT: usize = 2;

fn repeated(source: &str) -> Vec<(RepeatedKind, usize, usize)> {
    let mut parser = Parser::new();
    parser
        .set_language(&Language::TypeScript.grammar())
        .expect("la gramática carga");
    let tree = parser.parse(source, None).expect("el archivo se parsea");
    let mut found: Vec<(RepeatedKind, usize, usize)> =
        repeated_blocks(tree.root_node(), source.as_bytes(), SONAR_MIN_LINES_LIMIT)
            .into_iter()
            .map(|block| (block.kind, block.line, block.original_line))
            .collect();
    found.sort_by_key(|&(_, line, _)| line);
    found
}

#[test]
fn reports_the_later_copy_of_an_identical_function_at_its_name() {
    let source = "function first(values: number[]) {
  let total = 0;
  for (const value of values) total += value;
  return total;
}
const second = (values: number[]) => {
  let total = 0;
  for (const value of values) total += value;
  return total;
};
class Totals {
  third(values: number[]) {
    let total = 0;
    for (const value of values) total += value;
    return total;
  }
}
const ignoredInObject = {
  fourth(values: number[]) {
    let total = 0;
    for (const value of values) total += value;
    return total;
  },
};
";
    assert_eq!(
        repeated(source),
        vec![(RepeatedKind::Function, 6, 1), (RepeatedKind::Function, 12, 1)],
        "la flecha se marca en el =>, el método en su nombre y el del objeto no cuenta"
    );
}

#[test]
fn short_functions_and_parenthesized_bodies_follow_sonarjs() {
    let source = "const a = () => ({
  key: 1,
  other: 2,
  third: 3,
});
const b = (() => ({
  key: 1,
  other: 2,
  third: 3,
}));
const c = () => 1;
const d = () => 1;
";
    assert_eq!(
        repeated(source),
        vec![(RepeatedKind::Function, 6, 1)],
        "los paréntesis no cuentan y las llaves del objeto se descartan al medir"
    );
}

#[test]
fn reports_repeated_if_branches_only_when_they_span_lines() {
    let source = "if (a) {
  run(a);
  stop();
} else if (b) {
  run(a);
  stop();
} else {
  other();
}
if (c) { one(); } else if (d) { one(); } else { two(); }
";
    assert_eq!(repeated(source), vec![(RepeatedKind::Branch, 4, 1)]);
}

#[test]
fn identical_branches_without_else_are_all_reported_whatever_their_size() {
    assert_eq!(
        repeated("if (a) { one(); } else if (b) { one(); }\n"),
        vec![(RepeatedKind::Branch, 1, 1)]
    );
}

#[test]
fn reports_repeated_switch_cases_ignoring_the_trailing_break() {
    let source = "switch (kind) {
  case 1:
    log(kind);
    save(kind);
    break;
  case 2: {
    log(kind);
    save(kind);
  }
  case 3:
  default:
    other();
}
";
    assert_eq!(repeated(source), vec![(RepeatedKind::Branch, 6, 2)]);
}
