use super::LintFinding;
use super::tests::{lints, strict};
use crate::fix::apply;
use crate::limits::{JavaReference, Limits};
use crate::rules::RuleId;

const FIXTURE: &str = include_str!("fixtures/Pmd.java");
const GOLDEN: &str = include_str!("fixtures/Pmd.pmd");

fn like_pmd() -> Limits {
    strict().with_java_reference(JavaReference::Pmd)
}

fn is_pmd_rule(rule: RuleId) -> bool {
    [RuleId::RedundantModifier, RuleId::FinalClass].contains(&rule)
}

fn rendered_lines(findings: &[LintFinding]) -> Vec<String> {
    let mut lines: Vec<(usize, &str)> = findings
        .iter()
        .filter(|finding| is_pmd_rule(finding.kind.rule()))
        .map(|finding| (finding.line, finding.kind.rule().as_str()))
        .collect();
    lines.sort_unstable();
    lines.dedup();
    lines
        .into_iter()
        .map(|(line, rule)| format!("{line} {rule}"))
        .collect()
}

#[test]
fn modifiers_and_final_classes_match_pmd() {
    let golden: Vec<String> = GOLDEN.lines().map(str::to_owned).collect();
    assert_eq!(
        rendered_lines(&lints(FIXTURE, &like_pmd())),
        golden,
        "Pmd.java contra PMD 7.29"
    );
}

#[test]
fn checkstyle_stays_the_default_reference() {
    assert_ne!(
        rendered_lines(&lints(FIXTURE, &strict())),
        rendered_lines(&lints(FIXTURE, &like_pmd()))
    );
}

#[test]
fn every_pmd_fix_removes_its_own_problem() {
    let before = lints(FIXTURE, &like_pmd());
    let count = |findings: &[LintFinding], wanted: &LintFinding| {
        findings
            .iter()
            .filter(|other| other.kind.rule() == wanted.kind.rule() && other.subject == wanted.subject)
            .count()
    };
    for finding in before.iter().filter(|finding| is_pmd_rule(finding.kind.rule())) {
        let Some(fix) = &finding.fix else { continue };
        let fixed = apply(&fix.edits, FIXTURE).expect("el arreglo cae dentro del archivo");
        assert!(
            count(&lints(&fixed, &like_pmd()), finding) < count(&before, finding),
            "el arreglo de {finding:?} no lo resuelve"
        );
    }
}
