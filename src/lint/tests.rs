use tree_sitter::Parser;

use super::{LintFinding, LintKind, find_lints};
use crate::fix::apply;
use crate::language::Language;
use crate::limits::{Limits, Preset};
use crate::profile::LintStyle;
use crate::rules::RuleId;

const FIXTURES: [(&str, &str, &str); 7] = [
    (
        "Edges",
        include_str!("fixtures/Edges.java"),
        include_str!("fixtures/Edges.checkstyle"),
    ),
    (
        "Mods",
        include_str!("fixtures/Mods.java"),
        include_str!("fixtures/Mods.checkstyle"),
    ),
    (
        "Finals",
        include_str!("fixtures/Finals.java"),
        include_str!("fixtures/Finals.checkstyle"),
    ),
    (
        "Anon",
        include_str!("fixtures/Anon.java"),
        include_str!("fixtures/Anon.checkstyle"),
    ),
    (
        "Contexts",
        include_str!("fixtures/Contexts.java"),
        include_str!("fixtures/Contexts.checkstyle"),
    ),
    (
        "Two",
        include_str!("fixtures/Two.java"),
        include_str!("fixtures/Two.checkstyle"),
    ),
    (
        "Quirks",
        include_str!("fixtures/Quirks.java"),
        include_str!("fixtures/Quirks.checkstyle"),
    ),
];

fn strict() -> Limits {
    Preset::Strict.limits(Language::Java, false)
}

fn lints(source: &str, limits: Limits) -> Vec<LintFinding> {
    let mut parser = Parser::new();
    parser
        .set_language(&Language::Java.grammar())
        .expect("la gramática carga");
    let tree = parser.parse(source, None).expect("el archivo se parsea");
    find_lints(tree.root_node(), source.as_bytes(), LintStyle::Checkstyle, limits)
}

fn rendered(findings: &[LintFinding]) -> Vec<String> {
    let mut lines: Vec<String> = findings
        .iter()
        .map(|finding| {
            format!(
                "{}:{} {} {}",
                finding.line,
                finding.column,
                finding.kind.rule().as_str(),
                finding.subject
            )
        })
        .collect();
    lines.sort();
    lines
}

fn expected(golden: &str) -> Vec<String> {
    let mut lines: Vec<String> = golden.lines().map(str::to_owned).collect();
    lines.sort();
    lines
}

#[test]
fn reports_exactly_what_checkstyle_reports_with_sun_checks() {
    for (name, source, golden) in FIXTURES {
        assert_eq!(
            rendered(&lints(source, strict())),
            expected(golden),
            "{name}.java contra Checkstyle 14.3"
        );
    }
}

#[test]
fn every_fix_leaves_the_file_without_that_problem() {
    for (name, source, _) in FIXTURES {
        let before = lints(source, strict());
        let count = |findings: &[LintFinding], finding: &LintFinding| {
            findings
                .iter()
                .filter(|other| other.kind.rule() == finding.kind.rule() && other.subject == finding.subject)
                .count()
        };
        for finding in &before {
            let Some(fix) = &finding.fix else { continue };
            let fixed = apply(&fix.edits, source).expect("el arreglo cae dentro del archivo");
            assert!(
                count(&lints(&fixed, strict()), finding) < count(&before, finding),
                "{name}.java: el arreglo de {finding:?} no lo resuelve"
            );
        }
    }
}

#[test]
fn star_imports_have_no_mechanical_fix() {
    let source = "import java.util.*;\nclass A { List<String> names; }\n";
    let found = lints(source, strict());
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].kind, LintKind::StarImport);
    assert_eq!(
        found[0].fix, None,
        "elegir qué clases importar pide saber qué hay en el paquete"
    );
}

#[test]
fn columns_count_bytes_where_checkstyle_expands_tabs() {
    let source = "class A {\n\tvoid m(int value) {}\n}\n";
    let found = lints(source, strict());
    assert_eq!(rendered(&found), ["2:9 final-parameter value"]);
    assert_eq!(
        found[0].column, 9,
        "Checkstyle diría 16: expande el tab a 8 columnas; kerbo cuenta bytes como en el resto de las reglas"
    );
}

#[test]
fn respects_each_rule_switch_and_other_languages() {
    let source = "import a.B;\nclass A { void m(int value) {} }\n";
    let without_final = strict().with_rule(RuleId::FinalParameter, None);
    assert_eq!(rendered(&lints(source, without_final)), ["1:8 unused-import a.B"]);
    let everything_off = super::LINT_RULES
        .iter()
        .fold(strict(), |limits, &rule| limits.with_rule(rule, None));
    assert_eq!(lints(source, everything_off), Vec::new());
    let recommended = Preset::Recommended.limits(Language::Java, false);
    assert_eq!(rendered(&lints(source, recommended)), ["1:8 unused-import a.B"]);
    let mut parser = Parser::new();
    parser
        .set_language(&Language::Java.grammar())
        .expect("la gramática carga");
    let tree = parser.parse(source, None).expect("el archivo se parsea");
    let unmeasured = find_lints(
        tree.root_node(),
        source.as_bytes(),
        LintStyle::Unmeasured,
        strict(),
    );
    assert_eq!(unmeasured, Vec::new());
}
