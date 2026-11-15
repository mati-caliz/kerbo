use super::tests::{lints, strict};
use crate::rules::{JavaRule, RuleId};

const BODIES: &str = "class E {
    enum K { ONE, TWO }
    Object o = new Object() { int x; };
    void s(int k) {
        switch (k) { case 1: break; default: }
    }
    interface I { void m(); }
    record R(int a) { R { } }
    enum L {
        ONE { void f() { } },
        TWO { int x; };
        @interface Q { int v(); }
    }
}
";

#[test]
fn a_member_on_the_line_of_the_brace_needs_a_line_break_except_in_an_enum_body() {
    let found: Vec<(usize, usize)> = lints(BODIES, &strict())
        .into_iter()
        .filter(|finding| finding.kind.rule() == RuleId::Java(JavaRule::LeftCurly))
        .map(|finding| (finding.line, finding.column))
        .collect();
    assert_eq!(
        found,
        [(3, 29), (5, 20), (7, 17), (8, 21), (10, 13), (11, 13), (12, 22)]
    );
}
