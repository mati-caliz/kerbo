use super::tests::{lints, strict};
use crate::fix::apply;
use crate::rules::{JavaRule, RuleId};

const SOURCE: &str = "class Holder implements java.io.Serializable {
    private static transient Object cache;
    private transient Object state;
    static final transient int FIRST = 1, SECOND = 2;

    static final int twice(int value) {
        return value * 2;
    }

    final int instance() {
        return 0;
    }

    static int plain() {
        return 0;
    }

    Runnable task = new Runnable() {
        public void run() {
        }

        static final void helper() {
        }
    };
}

interface Shape {
    static int sides() {
        return 0;
    }
}
";

fn reported(rule: JavaRule) -> Vec<(usize, String)> {
    lints(SOURCE, &strict())
        .into_iter()
        .filter(|finding| finding.kind.rule() == RuleId::Java(rule))
        .map(|finding| (finding.line, finding.subject))
        .collect()
}

fn owned(expected: &[(usize, &str)]) -> Vec<(usize, String)> {
    expected
        .iter()
        .map(|(line, subject)| (*line, (*subject).to_owned()))
        .collect()
}

#[test]
fn reports_static_methods_declared_final() {
    assert_eq!(
        reported(JavaRule::FinalStaticMethod),
        owned(&[(6, "twice"), (22, "helper")])
    );
}

#[test]
fn reports_static_fields_declared_transient() {
    assert_eq!(
        reported(JavaRule::TransientStaticField),
        owned(&[(2, "cache"), (4, "FIRST, SECOND")])
    );
}

#[test]
fn the_fix_removes_the_modifier() {
    let fixed = lints(SOURCE, &strict())
        .into_iter()
        .filter(|finding| {
            [JavaRule::FinalStaticMethod, JavaRule::TransientStaticField]
                .into_iter()
                .any(|rule| finding.kind.rule() == RuleId::Java(rule))
        })
        .filter_map(|finding| finding.fix)
        .rev()
        .try_fold(SOURCE.to_owned(), |text, fix| apply(&fix.edits, &text))
        .expect("los arreglos caen dentro del archivo");
    assert!(fixed.contains("private static Object cache;"));
    assert!(fixed.contains("static final int FIRST = 1, SECOND = 2;"));
    assert!(fixed.contains("static int twice(int value)"));
    assert!(fixed.contains("static void helper()"));
}
