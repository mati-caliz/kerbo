use super::tests::{lints, strict};
use crate::limits::{JavaSettings, Limits};
use crate::rules::{JavaRule, RuleId};

const SOURCE: &str = r#"class Queries {
    String select = "select *\n" +
        "from orders\n" +
        "where id = ?";
    String one = "select *\n" + "from orders";
    String mixed = "select *\n" + name() + "from orders\n";
    String commented = "select *\n" + // the table
        "from orders\n";
    String inner = prefix() + ("a\n" + "b\n");
    String escaped = "a\\n" + "b\\n" + "c\n";
    String twice = "a\nb\n";
    String characters = "a" + '\n' + 1 + '\n';
    String blocks = """
        a
        """ + "b\n" + "c\n";
    String later = 1 + 2 + "a\n" + "b\n";

    String name() { return ""; }
    String prefix() { return ""; }
}
"#;

fn migrations(limits: &Limits) -> Vec<(usize, usize)> {
    lints(SOURCE, limits)
        .into_iter()
        .filter(|finding| finding.kind.rule() == RuleId::Java(JavaRule::TextBlockMigration))
        .map(|finding| (finding.line, finding.column))
        .collect()
}

#[test]
fn reports_concatenated_literals_with_two_line_breaks() {
    assert_eq!(migrations(&strict()), [(2, 30), (9, 34), (12, 32), (16, 30)]);
}

#[test]
fn needs_release_fifteen() {
    let java14 = strict().with_java(JavaSettings {
        release: Some(14),
        ..JavaSettings::default()
    });
    assert_eq!(migrations(&java14), []);
}
