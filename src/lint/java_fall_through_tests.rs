use super::tests::{lints, strict};
use crate::rules::{JavaRule, RuleId};

const SWITCHES: &str = "class Switches {
    int run(int kind, boolean flag) {
        switch (kind) {
            case 1:
                work();
            case 2:
                work();
                break;
            case 3:
                if (flag) {
                    return 1;
                }
            case 4:
                if (flag) {
                    return 1;
                } else {
                    throw new IllegalStateException();
                }
            case 5:
                work();
                // fall through
            case 6:
                work();
                //$FALL-THROUGH$
            case 7:
            case 8:
                try {
                    return 2;
                } finally {
                    work();
                }
            case 9:
                while (true) {
                    work();
                }
            case 10:
                work();
            default:
                work();
        }
        return 0;
    }

    void work() {
    }
}
";

const RETURNS: &str = "class Returns {
    boolean plain(boolean flag) {
        if (flag) {
            return true;
        } else {
            return false;
        }
    }

    boolean chained(Object item, String name) {
        if (item == null) {
            return true;
        }
        else if (name.isEmpty()) {
            // the candidate is fine
            return true;
        } else {
            return false;
        }
    }

    boolean other(boolean flag) {
        if (flag) {
            return true;
        } else {
            return flag;
        }
    }
}
";

fn reported(source: &str, rule: JavaRule) -> Vec<usize> {
    lints(source, &strict())
        .into_iter()
        .filter(|finding| finding.kind.rule() == RuleId::Java(rule))
        .map(|finding| finding.line)
        .collect()
}

#[test]
fn reports_cases_that_fall_through_without_a_comment() {
    assert_eq!(reported(SWITCHES, JavaRule::FallThrough), [6, 13, 25, 38]);
}

#[test]
fn reports_if_else_that_only_returns_booleans() {
    assert_eq!(reported(RETURNS, JavaRule::SimplifyBooleanReturn), [3, 14]);
}
