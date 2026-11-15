use super::tests::{lints, strict};
use crate::limits::{JavaSettings, Limits};
use crate::rules::{JavaRule, RuleId};

const SOURCE: &str = r#"class Bounds {
    static final int PAGE_SIZE = 50;

    long wait(Strategy strategy) {
        return Math.max(0, Math.min(60, strategy.check()));
    }

    int size(String limit) {
        return Math.min(Math.max(Integer.parseInt(limit), 1), PAGE_SIZE);
    }

    int constants() {
        return Math.max(0, Math.min(60, PAGE_SIZE));
    }

    int same(int a, int b) {
        return Math.max(a, Math.max(b, 3));
    }

    String text(int width, String dash) {
        StringBuilder line = new StringBuilder();
        line.append("-".repeat(width));
        line.append(dash.repeat(2)).append("=".repeat(width));
        new StringBuilder().append(dash);
        return line.toString();
    }

    interface Strategy {
        long check();
    }
}
"#;

fn reported(limits: &Limits, rule: JavaRule) -> Vec<(usize, usize)> {
    lints(SOURCE, limits)
        .into_iter()
        .filter(|finding| finding.kind.rule() == RuleId::Java(rule))
        .map(|finding| (finding.line, finding.column))
        .collect()
}

#[test]
fn reports_nested_min_and_max_unless_everything_inside_is_constant() {
    assert_eq!(
        reported(&strict(), JavaRule::MathClampMigration),
        [(5, 16), (9, 16)]
    );
}

#[test]
fn reports_a_repeated_string_appended_to_a_builder() {
    assert_eq!(
        reported(&strict(), JavaRule::StringRepeatCanBeUsed),
        [(22, 14), (23, 14), (23, 37)]
    );
}

#[test]
fn both_need_release_twenty_one() {
    let java17 = strict().with_java(JavaSettings {
        release: Some(17),
        ..JavaSettings::default()
    });
    assert_eq!(reported(&java17, JavaRule::MathClampMigration), []);
    assert_eq!(reported(&java17, JavaRule::StringRepeatCanBeUsed), []);
}

#[test]
fn reports_a_counting_loop_that_only_appends_the_same_value() {
    let source = "class Lines {\n    void pad(StringBuilder line, char c, int width, String[] parts) {\n        for (int i = 0; i < width; i++) {\n            line.append(c);\n        }\n        for (int i = width; i > 0; i--) line.append(' ');\n        for (int i = 0; i < width; i++) {\n            line.append(parts[i]);\n        }\n        for (int i = 0; i <= width; i++) {\n            line.append(c);\n        }\n        for (int i = 0; i < width; i++) {\n            line.append(c);\n            line.append(c);\n        }\n        for (long i = 0; i < width; i++) {\n            line.append(c);\n        }\n    }\n}\n";
    let found: Vec<usize> = lints(source, &strict())
        .into_iter()
        .filter(|finding| finding.kind.rule() == RuleId::Java(JavaRule::StringRepeatCanBeUsed))
        .map(|finding| finding.line)
        .collect();
    assert_eq!(found, [3, 6]);
}
