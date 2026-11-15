use super::tests::{lints, strict};
use crate::rules::{JavaRule, RuleId};

const SOURCE: &str = r#"/**
 * Generic over <N, E> nodes.
 */
public class J {
    /**
     * Compare.
     * <p/>
     * Natural ordering.
     * </p>
     *
     * @param that other
     * @return order
     */
    public int a(J that) {
        return 0;
    }

    /**
     * <expressions>
     * <expression>
     * <origin><![CDATA[
     * <repository>
     * <id>x</id>
     * </repository>
     * ]]></origin>
     * </expression>
     * <expressions>
     *
     * @return x
     */
    public int b() {
        return 0;
    }

    /**
     * Fine: <b>bold</b>, <p>para, {@code <N, E>}, a < b, <br>, <!-- c -->,
     * <a href="x.html">link</a>, <ul><li>one<li>two</ul>.
     *
     * @param x value
     * @return x
     */
    public int c(int x) {
        return x;
    }

    /**
     * Mismatched <b>bold</i> still parses.
     * @return x
     */
    public int d() {
        return 0;
    }

    /**
     * Stray </b> end.
     * @return x
     */
    public int e() {
        return 0;
    }

    /**
     * Attribute <a href=>broken</a>.
     * @return x
     */
    public int f() {
        return 0;
    }

    /**
     * Unclosed <ul><li>one</ul> and <p>nested <b>x</b>.
     * @return x
     */
    public int g() {
        return 0;
    }
}
"#;

#[test]
fn a_javadoc_that_checkstyle_cannot_parse_reports_the_error_line_instead_of_its_tags() {
    let found: Vec<(String, usize)> = lints(SOURCE, &strict())
        .into_iter()
        .filter(|finding| {
            [JavaRule::JavadocMethod, JavaRule::JavadocType]
                .iter()
                .any(|rule| finding.kind.rule() == RuleId::Java(*rule))
        })
        .map(|finding| (finding.kind.rule().as_str().to_owned(), finding.line))
        .collect();
    let expected: Vec<(String, usize)> = [
        ("javadoc-type", 2),
        ("javadoc-method", 9),
        ("javadoc-method", 26),
        ("javadoc-method", 47),
        ("javadoc-method", 55),
        ("javadoc-method", 63),
    ]
    .into_iter()
    .map(|(rule, line)| (rule.to_owned(), line))
    .collect();
    assert_eq!(found, expected);
}
