use super::tests::{lints, strict};
use crate::fix::apply;
use crate::limits::{JavaSettings, SourceEncoding};
use crate::rules::{JavaRule, RuleId};

const ESCAPE_MARK: &str = "%u";
const ESCAPE: &str = "\\u";
const TEMPLATE: &str = r#"class Escapes {
    String name = "Jos%u00e9";
    String kept = "%u00A0%u0301%u200B%uFFFD%u0000";
    String doubled = "\%u00e9";
    String marks = "%uuuu0041";
    String tab = "a%u0009b";
    char letter = '%u0042';
    String chained = "%u005cu0041";
    // fin%u000a
}
"#;

fn escaped(template: &str) -> String {
    template.replace(ESCAPE_MARK, ESCAPE)
}

fn unnecessary(source: &str) -> Vec<(usize, usize, String)> {
    lints(source, &strict())
        .into_iter()
        .filter(|finding| finding.kind.rule() == RuleId::Java(JavaRule::UnnecessaryUnicodeEscape))
        .map(|finding| (finding.line, finding.column, finding.subject))
        .collect()
}

#[test]
fn reports_the_escapes_intellij_finds_unnecessary() {
    let expected: Vec<(usize, usize, String)> = [
        (2, 23, "%u00e9"),
        (5, 21, "%uuuu0041"),
        (6, 20, "%u0009"),
        (7, 20, "%u0042"),
        (9, 11, "%u000a"),
    ]
    .into_iter()
    .map(|(line, column, escape)| (line, column, escaped(escape)))
    .collect();
    assert_eq!(unnecessary(&escaped(TEMPLATE)), expected);
}

#[test]
fn fixes_write_the_character_and_keep_a_tab_escaped_in_strings() {
    let source = escaped(TEMPLATE);
    let fixed = lints(&source, &strict())
        .into_iter()
        .filter(|finding| finding.kind.rule() == RuleId::Java(JavaRule::UnnecessaryUnicodeEscape))
        .filter_map(|finding| finding.fix)
        .rev()
        .try_fold(source.clone(), |text, fix| apply(&fix.edits, &text))
        .expect("los arreglos caen dentro del archivo");
    assert!(fixed.contains("\"Jos\u{e9}\""));
    assert!(fixed.contains(r#""A""#));
    assert!(fixed.contains(r#""a\tb""#));
    assert!(fixed.contains("'B'"));
    assert_eq!(unnecessary(&fixed), Vec::<(usize, usize, String)>::new());
}

#[test]
fn a_latin1_project_keeps_what_latin1_cannot_write_unless_the_file_is_already_utf8() {
    let latin1 = strict().with_java(JavaSettings {
        encoding: SourceEncoding::Latin1,
        ..JavaSettings::default()
    });
    let count = |template: &str| {
        lints(&escaped(template), &latin1)
            .into_iter()
            .filter(|finding| finding.kind.rule() == RuleId::Java(JavaRule::UnnecessaryUnicodeEscape))
            .count()
    };
    assert_eq!(count("class A { String s = \"%u00e9%u0141\"; }\n"), 1);
    assert_eq!(count("class A { String s = \"á%u00e9%u0141\"; }\n"), 2);
}
