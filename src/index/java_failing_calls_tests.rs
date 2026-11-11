use super::java_throws_tests::fixture_jdk;
use super::tests::summary_of;
use super::{IndexedSummary, project_lints};
use crate::jvm::library::Library;

const CHECKS: &str = r#"package app;

import org.apache.commons.lang3.Validate;

final class Checks {
    static int length(String text) {
        if (text == null) {
            throw new IllegalArgumentException();
        }
        return text.length();
    }

    static void require(boolean condition) {
        if (!condition) {
            throw new IllegalStateException();
        }
    }

    static int distance(String left, String right) {
        if (left == null || right == null) {
            throw new IllegalArgumentException();
        }
        return left.length() - right.length();
    }

    static int lenient(String text) {
        return text == null ? 0 : text.length();
    }

    void callers() {
        length(null);
        require(false);
        require(true);
        distance("a", null);
        distance(null, "b");
        lenient(null);
        Validate.isTrue(false, "never");
        Validate.notNull("value");
        org.junit.Assume.assumeTrue(false);
        try {
            length(null);
        } catch (IllegalArgumentException expected) {
        }
    }

    static void absent(Object value) {
        if (value != null) {
            throw new IllegalStateException();
        }
    }

    void guarded(String text, String other) {
        if (other != null && text != null) {
            absent(text);
        }
        if (text != null) {
            text = other;
            absent(text);
        }
        absent(text);
    }
}
"#;

fn findings(in_tests: bool) -> Vec<String> {
    let summary = summary_of(CHECKS);
    let files = [IndexedSummary {
        summary: &summary,
        in_tests,
    }];
    let library = Library::load(&[]).with_jdk(fixture_jdk());
    project_lints(&files, &library)
        .into_iter()
        .flatten()
        .filter(|finding| finding.kind.rule().as_str() == "failing-call")
        .map(|finding| format!("{}:{} {}", finding.line, finding.column, finding.subject))
        .collect()
}

#[test]
fn a_call_whose_arguments_meet_a_fail_contract_is_reported() {
    assert_eq!(
        findings(false),
        [
            "31:9 length",
            "32:9 require",
            "34:9 distance",
            "35:9 distance",
            "37:18 isTrue",
            "39:26 assumeTrue",
            "41:13 length",
            "54:13 absent",
        ]
    );
}

#[test]
fn a_test_that_expects_the_exception_is_not_reported() {
    assert_eq!(
        findings(true),
        [
            "31:9 length",
            "32:9 require",
            "34:9 distance",
            "35:9 distance",
            "37:18 isTrue",
            "39:26 assumeTrue",
            "54:13 absent",
        ]
    );
}
