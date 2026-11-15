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

const CHECKSTYLE_EDGES: &str = r#"import java.io.InputStream;
import org.junit.jupiter.api.io.TempDir;

public interface P {
    static Object load(InputStream in) throws Exception {
        class Local implements Runnable {
            @Override
            public void run() {
                System.out.println( "x" );
            }
        }
        for (;;) {
            break;
        }
        for (int i = 0; i < 3; ) {
            i++;
        }
        try (InputStream copy = in; ) {
            int value = (int) ( 1.5 );
            return foo( value ,
                1 );
        }
    }

    static Object foo(int a, int b) {
        return a;
    }

    class Holder {
        @TempDir
        java.nio.file.Path dir;
    }
}
"#;

#[test]
fn paren_pad_design_and_visibility_match_checkstyle_on_its_edges() {
    let found: Vec<(String, usize, usize)> = lints(CHECKSTYLE_EDGES, &strict())
        .into_iter()
        .filter(|finding| {
            [
                JavaRule::ParenPad,
                JavaRule::DesignForExtension,
                JavaRule::VisibilityModifier,
            ]
            .iter()
            .any(|rule| finding.kind.rule() == RuleId::Java(*rule))
        })
        .map(|finding| {
            (
                finding.kind.rule().as_str().to_owned(),
                finding.line,
                finding.column,
            )
        })
        .collect();
    let expected: Vec<(String, usize, usize)> = [
        ("design-for-extension", 7, 13),
        ("paren-pad", 9, 35),
        ("paren-pad", 9, 41),
        ("paren-pad", 19, 31),
        ("paren-pad", 19, 37),
        ("paren-pad", 20, 23),
        ("paren-pad", 21, 19),
        ("visibility-modifier", 31, 28),
    ]
    .into_iter()
    .map(|(rule, line, column)| (rule.to_owned(), line, column))
    .collect();
    assert_eq!(found, expected);
}
