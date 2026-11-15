use super::tests::summary_of;
use super::{IndexedSummary, project_lints};
use crate::jvm::library::Library;
use crate::lint::java_sequenced_collection::SequencedCandidate;
use std::path::Path;

const SOURCE: &str = "package app;

import sample.cli.Names;
import sample.cli.Argument;

class Uses {
    String first(Names names, Argument argument) {
        return names.get(0);
    }
}
";

fn candidate(type_name: &str, line: usize) -> SequencedCandidate {
    SequencedCandidate {
        type_name: type_name.to_owned(),
        needs_list: true,
        method: "getFirst".to_owned(),
        line,
        column: 22,
    }
}

#[test]
fn a_library_type_whose_parents_include_a_list_has_the_sequenced_methods() {
    let jar = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/jvm/fixtures/annotated.jar");
    let mut summary = summary_of(SOURCE);
    summary.sequenced_candidates = vec![
        candidate("Names", 8),
        candidate("Argument", 9),
        candidate("Unknown", 10),
    ];
    let files = [IndexedSummary {
        summary: &summary,
        in_tests: false,
    }];
    let found: Vec<String> = project_lints(&files, &Library::load(&[jar]))
        .into_iter()
        .flatten()
        .filter(|finding| finding.kind.rule().as_str() == "sequenced-collection-method")
        .map(|finding| format!("{}:{} {}", finding.line, finding.column, finding.subject))
        .collect();
    assert_eq!(found, ["8:22 getFirst"]);
}
