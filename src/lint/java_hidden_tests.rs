use super::tests::{lints, strict};
use crate::rules::{JavaRule, RuleId};

const SOURCE: &str = "class W {
    boolean f(boolean a, boolean b, boolean c) {
        return a && (
                b || c);
    }

    boolean g(boolean a, boolean b) {
        return a &&
                b;
    }

    boolean h(boolean a, boolean b) {
        return (a
                ) && b;
    }

    record P(int x, int y) {
        P(int x, String label, int y) {
            this(x, y);
        }

        P(int x, int y) {
            this.x = x;
            this.y = y;
        }

        static P of(int x) {
            return new P(x, 0);
        }

        int sum(int y) {
            return x + y;
        }
    }
}
";

fn positions(rule: JavaRule) -> Vec<(usize, usize)> {
    lints(SOURCE, &strict())
        .into_iter()
        .filter(|finding| finding.kind.rule() == RuleId::Java(rule))
        .map(|finding| (finding.line, finding.column))
        .collect()
}

#[test]
fn a_parenthesis_on_the_line_of_the_operator_counts_as_its_operand() {
    assert_eq!(positions(JavaRule::OperatorWrap), [(8, 18)]);
}

#[test]
fn record_components_are_fields_that_constructor_and_method_parameters_hide() {
    assert_eq!(
        positions(JavaRule::HiddenField),
        [(18, 15), (18, 36), (22, 15), (22, 22), (31, 21)]
    );
}
