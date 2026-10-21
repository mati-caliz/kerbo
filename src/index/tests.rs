use std::path::Path;

use tree_sitter::Parser;

use super::java::{JavaSummary, summarize};
use super::{IndexedSummary, project_lints};
use crate::jvm::library::Library;
use crate::language::Language;

const FILTER: &str = "package app;\n\nimport java.util.List;\nimport sample.web.Filter;\n\nclass AuditFilter extends Filter {\n    @Override\n    protected void handle(String request, Object handler, List<String> names) {\n        if (names.isEmpty()) {\n            return;\n        }\n    }\n\n    @Override\n    public int compareTo(Filter other) {\n        return 0;\n    }\n}\n";

fn summary_of(source: &str) -> JavaSummary {
    let mut parser = Parser::new();
    parser
        .set_language(&Language::Java.grammar().expect("tiene gramática"))
        .expect("la gramática carga");
    let tree = parser.parse(source, None).expect("se parsea");
    summarize(tree.root_node(), source.as_bytes())
}

#[test]
fn an_unannotated_override_of_a_null_marked_library_is_reported_unless_inferred() {
    let jar = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/jvm/fixtures/sample.jar");
    let library = Library::load(&[jar]);
    let summary = summary_of(FILTER);
    let files = [IndexedSummary {
        summary: &summary,
        in_tests: false,
    }];
    let found: Vec<String> = project_lints(&files, &library)
        .into_iter()
        .flatten()
        .filter(|finding| finding.kind.rule().as_str() == "nullable-problems")
        .map(|finding| format!("{}:{} {}", finding.line, finding.column, finding.subject))
        .collect();
    assert_eq!(found, ["8:34 request @NullMarked"]);
    let without_library = project_lints(&files, &Library::load(&[]));
    assert!(
        without_library
            .iter()
            .flatten()
            .all(|finding| finding.kind.rule().as_str() != "nullable-problems")
    );
}
