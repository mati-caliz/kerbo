use tree_sitter::Parser;

use crate::analysis::SourceUnit;
use crate::fix::apply;
use crate::language::Language;
use crate::limits::{Limits, Preset};
use crate::lint::{LintFinding, find_lints};
use crate::profile::LintStyle;
use crate::rules::{EslintRule, RuleId};

const FATAL_MARK: &str = "FATAL";
const FIXTURES: [(&str, Language, &str, &str); 27] = [
    (
        "hashbang",
        Language::JavaScript,
        include_str!("fixtures/hashbang.js"),
        include_str!("fixtures/hashbang.eslint"),
    ),
    (
        "escapes",
        Language::JavaScript,
        include_str!("fixtures/escapes.js"),
        include_str!("fixtures/escapes.eslint"),
    ),
    (
        "expressions",
        Language::TypeScript,
        include_str!("fixtures/expressions.ts"),
        include_str!("fixtures/expressions.eslint"),
    ),
    (
        "vars",
        Language::JavaScript,
        include_str!("fixtures/vars.js"),
        include_str!("fixtures/vars.eslint"),
    ),
    (
        "redeclare",
        Language::JavaScript,
        include_str!("fixtures/redeclare.js"),
        include_str!("fixtures/redeclare.eslint"),
    ),
    (
        "wrappers",
        Language::TypeScript,
        include_str!("fixtures/wrappers.ts"),
        include_str!("fixtures/wrappers.eslint"),
    ),
    (
        "finally",
        Language::JavaScript,
        include_str!("fixtures/finally.js"),
        include_str!("fixtures/finally.eslint"),
    ),
    (
        "spread",
        Language::JavaScript,
        include_str!("fixtures/spread.js"),
        include_str!("fixtures/spread.eslint"),
    ),
    (
        "applies",
        Language::TypeScript,
        include_str!("fixtures/applies.ts"),
        include_str!("fixtures/applies.eslint"),
    ),
    (
        "conditions",
        Language::JavaScript,
        include_str!("fixtures/conditions.js"),
        include_str!("fixtures/conditions.eslint"),
    ),
    (
        "binaries",
        Language::JavaScript,
        include_str!("fixtures/binaries.js"),
        include_str!("fixtures/binaries.eslint"),
    ),
    (
        "assignments",
        Language::JavaScript,
        include_str!("fixtures/assignments.js"),
        include_str!("fixtures/assignments.eslint"),
    ),
    (
        "declared",
        Language::TypeScript,
        include_str!("fixtures/declared.ts"),
        include_str!("fixtures/declared.eslint"),
    ),
    (
        "checks",
        Language::JavaScript,
        include_str!("fixtures/checks.js"),
        include_str!("fixtures/checks.eslint"),
    ),
    (
        "errors",
        Language::JavaScript,
        include_str!("fixtures/errors.js"),
        include_str!("fixtures/errors.eslint"),
    ),
    (
        "namespaces",
        Language::TypeScript,
        include_str!("fixtures/namespaces.ts"),
        include_str!("fixtures/namespaces.eslint"),
    ),
    (
        "requires",
        Language::TypeScript,
        include_str!("fixtures/requires.ts"),
        include_str!("fixtures/requires.eslint"),
    ),
    (
        "bindings",
        Language::JavaScript,
        include_str!("fixtures/bindings.js"),
        include_str!("fixtures/bindings.eslint"),
    ),
    (
        "ambient",
        Language::TypeScript,
        include_str!("fixtures/ambient.ts"),
        include_str!("fixtures/ambient.eslint"),
    ),
    (
        "unused",
        Language::JavaScript,
        include_str!("fixtures/unused.js"),
        include_str!("fixtures/unused.eslint"),
    ),
    (
        "declarations",
        Language::TypeScript,
        include_str!("fixtures/declarations.ts"),
        include_str!("fixtures/declarations.eslint"),
    ),
    (
        "statements",
        Language::JavaScript,
        include_str!("fixtures/statements.js"),
        include_str!("fixtures/statements.eslint"),
    ),
    (
        "types",
        Language::TypeScript,
        include_str!("fixtures/types.ts"),
        include_str!("fixtures/types.eslint"),
    ),
    (
        "grammar",
        Language::JavaScript,
        include_str!("fixtures/grammar.jsx"),
        include_str!("fixtures/grammar.eslint"),
    ),
    (
        "variance",
        Language::TypeScript,
        include_str!("fixtures/variance.ts"),
        include_str!("fixtures/variance.eslint"),
    ),
    (
        "annotated",
        Language::JavaScript,
        include_str!("fixtures/annotated.js"),
        include_str!("fixtures/annotated.eslint"),
    ),
    (
        "flow",
        Language::JavaScript,
        include_str!("fixtures/flow.js"),
        include_str!("fixtures/flow.eslint"),
    ),
];

fn strict(language: Language) -> Limits {
    Preset::Strict.limits(language, false)
}

pub fn lints(language: Language, source: &str, limits: Limits) -> Vec<LintFinding> {
    let mut parser = Parser::new();
    parser
        .set_language(&language.grammar().expect("tiene gramática"))
        .expect("la gramática carga");
    let tree = parser.parse(source, None).expect("el archivo se parsea");
    let unit = SourceUnit {
        language,
        source: source.as_bytes(),
        path: None,
        crate_macros: None,
    };
    find_lints(tree.root_node(), unit, LintStyle::Eslint, limits)
}

fn rendered(findings: &[LintFinding]) -> Vec<String> {
    let mut lines: Vec<String> = findings
        .iter()
        .map(|finding| {
            format!(
                "{}:{} {}",
                finding.line,
                finding.column,
                finding.kind.rule().as_str()
            )
        })
        .collect();
    lines.sort();
    lines
}

fn expected(golden: &str) -> Vec<String> {
    let mut lines: Vec<String> = golden
        .lines()
        .filter(|line| !line.starts_with(FATAL_MARK))
        .map(str::to_owned)
        .collect();
    lines.sort();
    lines
}

#[test]
fn every_fixture_matches_eslint() {
    for (name, language, source, golden) in FIXTURES {
        assert_eq!(
            rendered(&lints(language, source, strict(language))),
            expected(golden),
            "{name}"
        );
    }
}

const FIX_FIXTURES: [(&str, Language, &str, EslintRule, &str); 4] = [
    (
        "wrappers",
        Language::TypeScript,
        include_str!("fixtures/wrappers.ts"),
        EslintRule::NoWrapperObjectTypes,
        include_str!("fixtures/wrappers.no-wrapper-object-types.fixed"),
    ),
    (
        "bindings",
        Language::JavaScript,
        include_str!("fixtures/bindings.js"),
        EslintRule::PreferConst,
        include_str!("fixtures/bindings.prefer-const.fixed"),
    ),
    (
        "bindings",
        Language::JavaScript,
        include_str!("fixtures/bindings.js"),
        EslintRule::NoVar,
        include_str!("fixtures/bindings.no-var.fixed"),
    ),
    (
        "vars",
        Language::JavaScript,
        include_str!("fixtures/vars.js"),
        EslintRule::NoVar,
        include_str!("fixtures/vars.no-var.fixed"),
    ),
];

#[test]
fn every_fix_matches_eslint_fix() {
    for (name, language, source, rule, fixed) in FIX_FIXTURES {
        let mut edits: Vec<_> = lints(language, source, strict(language))
            .into_iter()
            .filter(|finding| finding.kind.rule() == RuleId::Eslint(rule))
            .filter_map(|finding| finding.fix)
            .flat_map(|fix| fix.edits)
            .collect();
        edits.sort_by_key(|edit| edit.start);
        edits.dedup_by(|later, earlier| later.start < earlier.end);
        assert_eq!(apply(&edits, source).as_deref(), Some(fixed), "{name}");
    }
}

#[test]
fn a_useless_escape_loses_its_backslash() {
    let source = "export const text = \"a\\-b\";\nexport const pattern = /[\\/]/;\n";
    let edits: Vec<_> = lints(Language::JavaScript, source, strict(Language::JavaScript))
        .into_iter()
        .filter_map(|finding| finding.fix)
        .flat_map(|fix| fix.edits)
        .collect();
    assert_eq!(
        apply(&edits, source).as_deref(),
        Some("export const text = \"a-b\";\nexport const pattern = /[/]/;\n")
    );
}

#[test]
fn a_file_that_does_not_parse_is_not_linted() {
    assert_eq!(
        lints(
            Language::JavaScript,
            "debugger; if (",
            strict(Language::JavaScript)
        ),
        Vec::new()
    );
}

#[test]
fn flow_syntax_that_typescript_rejects_is_not_linted() {
    let snippets = [
        "import typeof Shape from './shape';\ndebugger;\n",
        "export function pick<T: Base>(value: T) {}\ndebugger;\n",
        "type Exact = {| name: string |};\ndebugger;\n",
        "declare module.exports: Api;\ndebugger;\n",
        "const cache = new Map<string, number>([]);\ndebugger;\n",
    ];
    for snippet in snippets {
        assert_eq!(
            lints(Language::JavaScript, snippet, strict(Language::JavaScript)),
            Vec::new(),
            "{snippet}"
        );
    }
}

#[test]
fn rules_turned_off_are_not_reported() {
    let limits = strict(Language::JavaScript).with_rule(RuleId::Eslint(EslintRule::NoDebugger), None);
    assert_eq!(lints(Language::JavaScript, "debugger;", limits), Vec::new());
    let silent = EslintRule::ALL.into_iter().fold(limits, |limits, rule| {
        limits.with_rule(RuleId::Eslint(rule), None)
    });
    assert_eq!(lints(Language::JavaScript, "if (a) {}", silent), Vec::new());
}

#[test]
fn typescript_leaves_to_the_compiler_what_it_checks() {
    let source = "export const keys = { a: 1, a: 2 };";
    assert_eq!(
        rendered(&lints(Language::JavaScript, source, strict(Language::JavaScript))),
        ["1:29 no-dupe-keys"]
    );
    assert_eq!(
        lints(Language::TypeScript, source, strict(Language::TypeScript)),
        Vec::new()
    );
}

#[test]
fn looser_presets_drop_the_strict_and_stylistic_rules() {
    let source = "export const value: any = input!;";
    let recommended = Preset::Recommended.limits(Language::TypeScript, false);
    assert_eq!(
        rendered(&lints(Language::TypeScript, source, recommended)),
        ["1:21 no-explicit-any"]
    );
    let minimal = Preset::Minimal.limits(Language::TypeScript, false);
    assert_eq!(lints(Language::TypeScript, source, minimal), Vec::new());
}

#[test]
fn a_debugger_in_a_statement_list_is_removed() {
    let source = "export function run() {\n  debugger;\n  return 1;\n}\nif (ready) debugger;\n";
    let findings = lints(Language::JavaScript, source, strict(Language::JavaScript));
    let fixes: Vec<_> = findings.iter().map(|finding| finding.fix.as_ref()).collect();
    let removal = fixes.first().copied().flatten().expect("el primero se arregla");
    assert_eq!(
        apply(&removal.edits, source).as_deref(),
        Some("export function run() {\n  return 1;\n}\nif (ready) debugger;\n")
    );
    assert_eq!(fixes.get(1).copied().flatten(), None);
}
