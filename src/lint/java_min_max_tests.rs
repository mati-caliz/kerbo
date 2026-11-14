use super::tests::{lints, strict};
use crate::fix::apply;
use crate::rules::{JavaRule, RuleId};

const SOURCE: &str = "class Bounds {
    int count;

    int ternary(int limit) {
        return (limit < count) ? limit : count;
    }

    double clamp(double value) {
        if (value >= 1.0) {
            return 1.0;
        }
        return value;
    }

    long assigned(long a, long b) {
        long result;
        if (a > b) {
            result = a;
        } else {
            result = b;
        }
        return result;
    }

    int declared(int a, int b) {
        int smallest = a;
        if (b < a) smallest = b;
        return smallest;
    }

    int mixed(int a, long b) {
        return a < b ? a : (int) b;
    }

    int unrelated(int a, int b, int c) {
        return a < b ? c : a;
    }
}
";

fn findings() -> Vec<(usize, String)> {
    lints(SOURCE, &strict())
        .into_iter()
        .filter(|finding| finding.kind.rule() == RuleId::Java(JavaRule::ManualMinMaxCalculation))
        .map(|finding| (finding.line, finding.subject))
        .collect()
}

#[test]
fn reports_hand_written_minimums_and_maximums() {
    let expected: Vec<(usize, String)> = [(5, "min"), (9, "min"), (17, "max"), (27, "min")]
        .into_iter()
        .map(|(line, function)| (line, function.to_owned()))
        .collect();
    assert_eq!(findings(), expected);
}

#[test]
fn the_fix_calls_math() {
    let fixed = lints(SOURCE, &strict())
        .into_iter()
        .filter(|finding| finding.kind.rule() == RuleId::Java(JavaRule::ManualMinMaxCalculation))
        .filter_map(|finding| finding.fix)
        .rev()
        .try_fold(SOURCE.to_owned(), |text, fix| apply(&fix.edits, &text))
        .expect("los arreglos caen dentro del archivo");
    assert!(fixed.contains("return Math.min(limit, count);"));
    assert!(fixed.contains("return Math.min(value, 1.0);"));
    assert!(fixed.contains("result = Math.max(a, b);"));
    assert!(fixed.contains("int smallest = Math.min(b, a);"));
}
