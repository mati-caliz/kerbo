use super::tests::{lints, strict};
use crate::rules::{JavaRule, RuleId};

const SOURCE: &str = "class Semis {
    ;
    int a;;
    enum Kind {
        A, B;
        ;
        void run() { }
        ;
    }
    enum Empty {
        ;
    }
    enum Twice {
        A;;
    }
    interface Shape {
        ;
    }
}
";

#[test]
fn stray_semicolons_in_type_bodies_are_empty_statements_like_checkstyle() {
    let reported: Vec<(usize, usize)> = lints(SOURCE, &strict())
        .into_iter()
        .filter(|finding| finding.kind.rule() == RuleId::Java(JavaRule::EmptyStatement))
        .map(|finding| (finding.line, finding.column))
        .collect();
    assert_eq!(reported, [(2, 5), (3, 11), (6, 9), (8, 9), (14, 11), (17, 9)]);
}
