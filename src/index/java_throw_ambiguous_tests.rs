use super::java_throws_tests::fixture_jdk;
use super::tests::summary_of;
use super::{IndexedSummary, project_lints};
use crate::jvm::library::Library;

const AMBIGUOUS: &str = r"package app.project;

public class CycleDetectedException extends Exception {
}
";

const AMBIGUOUS_UNCHECKED: &str = r"package app.internal;

class Graph {
    static class CycleDetectedException extends RuntimeException {
    }
}
";

const AMBIGUOUS_CALLER: &str = r#"package app.project;

class GraphTest {
    static void addEdge(String from, String to) throws CycleDetectedException {
        if (from.equals(to)) {
            throw new CycleDetectedException();
        }
    }

    void cycles() throws Exception {
        addEdge("a", "b");
    }
}
"#;

// Con dos tipos del proyecto del mismo nombre y padres distintos (uno chequeado y otro no), la
// jerarquía no se sabe: no se marca el `throws Exception` de quien llama.
#[test]
fn an_ambiguous_project_exception_is_not_judged() {
    assert_eq!(
        redundant_in_files(&[AMBIGUOUS, AMBIGUOUS_UNCHECKED, AMBIGUOUS_CALLER]),
        Vec::<String>::new()
    );
}

fn redundant_in_files(sources: &[&str]) -> Vec<String> {
    let summaries: Vec<_> = sources.iter().map(|source| summary_of(source)).collect();
    let files: Vec<_> = summaries
        .iter()
        .map(|summary| IndexedSummary {
            summary,
            in_tests: false,
        })
        .collect();
    let library = Library::load(&[]).with_jdk(fixture_jdk());
    project_lints(&files, &library)
        .into_iter()
        .flatten()
        .filter(|finding| finding.kind.rule().as_str() == "redundant-throws")
        .map(|finding| format!("{}:{} {}", finding.line, finding.column, finding.subject))
        .collect()
}
