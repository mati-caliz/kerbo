use tree_sitter::Parser;

use crate::analysis::SourceUnit;
use crate::fix::apply;
use crate::language::Language;
use crate::limits::{Limits, Preset};
use crate::lint::{LintFinding, find_lints};
use crate::profile::LintStyle;
use crate::rules::{EslintRule, RuleId};

mod fixtures;

use fixtures::FIXTURES;

const FATAL_MARK: &str = "FATAL";

fn strict(language: Language) -> Limits {
    Preset::Strict.limits(language, false)
}

fn strict_lints(language: Language, source: &str) -> Vec<LintFinding> {
    lints(language, source, &strict(language))
}

pub fn lints(language: Language, source: &str, limits: &Limits) -> Vec<LintFinding> {
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
        go_platform: None,
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
            rendered(&strict_lints(language, source)),
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
        let mut edits: Vec<_> = strict_lints(language, source)
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
    let edits: Vec<_> = strict_lints(Language::JavaScript, source)
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
            &strict(Language::JavaScript)
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
            strict_lints(Language::JavaScript, snippet),
            Vec::new(),
            "{snippet}"
        );
    }
}

#[test]
fn rules_turned_off_are_not_reported() {
    let limits = strict(Language::JavaScript).with_rule(RuleId::Eslint(EslintRule::NoDebugger), None);
    assert_eq!(lints(Language::JavaScript, "debugger;", &limits), Vec::new());
    let silent = EslintRule::ALL.into_iter().fold(limits, |limits, rule| {
        limits.with_rule(RuleId::Eslint(rule), None)
    });
    assert_eq!(lints(Language::JavaScript, "if (a) {}", &silent), Vec::new());
}

#[test]
fn typescript_leaves_to_the_compiler_what_it_checks() {
    let source = "export const keys = { a: 1, a: 2 };";
    assert_eq!(
        rendered(&strict_lints(Language::JavaScript, source)),
        ["1:29 no-dupe-keys"]
    );
    assert_eq!(strict_lints(Language::TypeScript, source), Vec::new());
}

#[test]
fn looser_presets_drop_the_strict_and_stylistic_rules() {
    let source = "export const value: any = input!;";
    let recommended = Preset::Recommended.limits(Language::TypeScript, false);
    assert_eq!(
        rendered(&lints(Language::TypeScript, source, &recommended)),
        ["1:21 no-explicit-any"]
    );
    let minimal = Preset::Minimal.limits(Language::TypeScript, false);
    assert_eq!(lints(Language::TypeScript, source, &minimal), Vec::new());
}

#[test]
fn a_debugger_in_a_statement_list_is_removed() {
    let source = "export function run() {\n  debugger;\n  return 1;\n}\nif (ready) debugger;\n";
    let findings = strict_lints(Language::JavaScript, source);
    let fixes: Vec<_> = findings.iter().map(|finding| finding.fix.as_ref()).collect();
    let removal = fixes.first().copied().flatten().expect("el primero se arregla");
    assert_eq!(
        apply(&removal.edits, source).as_deref(),
        Some("export function run() {\n  return 1;\n}\nif (ready) debugger;\n")
    );
    assert_eq!(fixes.get(1).copied().flatten(), None);
}
