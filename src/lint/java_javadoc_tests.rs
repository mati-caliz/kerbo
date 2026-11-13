use super::java_style::JAVADOC_RULES;
use super::tests::{lints, strict};
use crate::rules::RuleId;

fn rendered_lines(source: &str) -> Vec<String> {
    let mut lines: Vec<(usize, String)> = lints(source, &strict())
        .into_iter()
        .filter(|finding| matches!(finding.kind.rule(), RuleId::Java(rule) if JAVADOC_RULES.contains(&rule)))
        .map(|finding| {
            (
                finding.line,
                format!("{} {}", finding.line, finding.kind.rule().as_str()),
            )
        })
        .collect();
    lines.sort();
    lines.into_iter().map(|(_, line)| line).collect()
}

#[test]
fn javadoc_rules_match_checkstyle() {
    let golden: Vec<String> = include_str!("fixtures/Javadoc.javadoc")
        .lines()
        .map(str::to_owned)
        .collect();
    assert_eq!(
        rendered_lines(include_str!("fixtures/Javadoc.java")),
        golden,
        "Javadoc.java contra Checkstyle 14.4"
    );
}

#[test]
fn javadoc_rules_are_off_in_recommended() {
    let source = "public class Plain {\n    public int count;\n}\n";
    let recommended = crate::limits::Preset::Recommended.limits(crate::language::Language::Java, false);
    assert!(
        lints(source, &recommended).iter().all(
            |finding| !matches!(finding.kind.rule(), RuleId::Java(rule) if JAVADOC_RULES.contains(&rule))
        ),
        "recommended no pide Javadoc"
    );
    assert!(!rendered_lines(source).is_empty(), "strict sí");
}
