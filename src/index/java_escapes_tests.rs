use super::tests::summary_of;
use super::{IndexedSummary, project_lints};
use crate::jvm::library::Library;

const HANDLER: &str = r"package app.orders;

import java.util.List;

public class OrderHandler {
    private static class Step {}

    public void apply(OrderState state, List<Entry> entries) {}

    void internal(OrderState state) {}

    protected Step next() { return null; }

    public <OrderState> OrderState generic(OrderState value) { return value; }

    private void hidden(Step step) {}

    public Shared shared() { return null; }
}

class Entry {}
";

const STATE: &str = r"package app.orders;

class OrderState {}
";

const SHARED: &str = r"package app.orders;

public class Shared {}
";

const OTHER: &str = r"package app.billing;

import app.orders.Shared;

public class Invoice {
    public Entry entry() { return null; }

    public Shared shared() { return null; }
}
";

#[test]
fn a_less_visible_type_in_a_more_visible_signature_is_reported() {
    let sources = [HANDLER, STATE, SHARED, OTHER];
    let summaries: Vec<_> = sources.iter().map(|source| summary_of(source)).collect();
    let files: Vec<IndexedSummary<'_>> = summaries
        .iter()
        .map(|summary| IndexedSummary {
            summary,
            in_tests: false,
        })
        .collect();
    let found: Vec<String> = project_lints(&files, &Library::load(&[]))
        .into_iter()
        .enumerate()
        .flat_map(|(file, findings)| {
            findings
                .into_iter()
                .filter(|finding| finding.kind.rule().as_str() == "class-escapes-its-scope")
                .map(move |finding| format!("{file} {}:{} {}", finding.line, finding.column, finding.subject))
        })
        .collect();
    assert_eq!(found, ["0 8:23 OrderState", "0 8:46 Entry", "0 12:15 Step"]);
}

const SERVICE: &str = r"package app.cases;

import java.io.ObjectInputStream;
import java.util.Comparator;

public class CaseService {
    public void fail(String caseId, String reason) {
        System.out.println(caseId);
    }

    public static void main(String[] args) {}

    private void readObject(ObjectInputStream stream) {}

    @org.junit.jupiter.params.ParameterizedTest
    void parameterized(String value) {}

    public record Summary(String name, int total) {}
}

class ByName implements Comparator<String> {
    public int compare(String first, String second) { return 0; }
}

class Listener {
    void on(String event) {}
}

class LoudListener extends Listener {
    void on(String event) {}
}
";

#[test]
fn a_parameter_the_body_never_reads_is_reported() {
    let summary = summary_of(SERVICE);
    let files = [IndexedSummary {
        summary: &summary,
        in_tests: false,
    }];
    let found: Vec<String> = project_lints(&files, &Library::load(&[]))
        .into_iter()
        .flatten()
        .filter(|finding| finding.kind.rule().as_str() == "unused-parameter")
        .map(|finding| format!("{}:{} {}", finding.line, finding.column, finding.subject))
        .collect();
    assert_eq!(found, ["7:44 reason"]);
}

const LINKS: &str = r"package app.links;

class Links {
    private boolean isValid(String token) { return !token.isEmpty(); }

    private boolean isExpired(String token) { return token.isEmpty(); }

    private boolean isKnown(String token) { return token.isEmpty(); }

    void check(String first, String second) {
        if (!isValid(first) || !(isValid(second))) {
            return;
        }
        if (!isExpired(first)) {
            return;
        }
        if (!isKnown(first) && isKnown(second)) {
            return;
        }
    }
}
";

#[test]
fn a_boolean_method_every_call_negates_is_reported() {
    let summary = summary_of(LINKS);
    let files = [IndexedSummary {
        summary: &summary,
        in_tests: false,
    }];
    let found: Vec<String> = project_lints(&files, &Library::load(&[]))
        .into_iter()
        .flatten()
        .filter(|finding| finding.kind.rule().as_str() == "boolean-method-is-always-inverted")
        .map(|finding| format!("{}:{} {}", finding.line, finding.column, finding.subject))
        .collect();
    assert_eq!(found, ["4:21 isValid"]);
}
