use super::java_throws_tests::fixture_jdk;
use super::tests::summary_of;
use super::{IndexedSummary, project_lints};
use crate::jvm::library::Library;

const MODEL: &str = "package app;

import java.util.List;
import java.util.Set;

class Result {
    List<String> getProblems() {
        return null;
    }

    Counter getTotals() {
        return null;
    }

    String[] getNames() {
        return null;
    }
}

class Other {
    Set<String> getProblems() {
        return null;
    }

    List<String> getNames() {
        return null;
    }
}

class Counter {
    int size() {
        return 0;
    }
}
";

const USES: &str = "package app;

class Uses {
    boolean check(Result result) {
        return result.getProblems().size() > 0
            && result.getTotals().size() == 0
            && result.getNames().size() == 0;
    }
}
";

#[test]
fn a_size_on_a_chain_that_resolves_to_a_collection_is_reported() {
    let summaries = [summary_of(MODEL), summary_of(USES)];
    let files: Vec<IndexedSummary<'_>> = summaries
        .iter()
        .map(|summary| IndexedSummary {
            summary,
            in_tests: false,
        })
        .collect();
    let found: Vec<(usize, String)> = project_lints(&files, &Library::load(&[]).with_jdk(fixture_jdk()))
        .into_iter()
        .flatten()
        .filter(|finding| finding.kind.rule().as_str() == "size-replaceable-by-is-empty")
        .map(|finding| (finding.line, finding.subject))
        .collect();
    assert_eq!(found, [(5, "!result.getProblems().isEmpty()".to_owned())]);
}
