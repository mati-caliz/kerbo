use std::path::Path;

use tree_sitter::Parser;

use super::java::{JavaSummary, summarize};
use super::{IndexedSummary, project_lints};
use crate::jvm::library::Library;
use crate::language::Language;

const FILTER: &str = "package app;\n\nimport java.util.List;\nimport sample.web.Filter;\n\nclass AuditFilter extends Filter {\n    @Override\n    protected void handle(String request, Object handler, List<String> names) {\n        if (names.isEmpty()) {\n            return;\n        }\n    }\n\n    @Override\n    public int compareTo(Filter other) {\n        return 0;\n    }\n\n    @Override\n    public String name() {\n        return \"audit\";\n    }\n\n    @Override\n    public Object find(String key) {\n        return null;\n    }\n}\n";

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
    assert_eq!(
        found,
        [
            "8:34 request @NullMarked",
            "20:19 name() @NullMarked",
            "25:31 key @NullMarked"
        ]
    );
    let without_library = project_lints(&files, &Library::load(&[]));
    assert!(
        without_library
            .iter()
            .flatten()
            .all(|finding| finding.kind.rule().as_str() != "nullable-problems")
    );
}

const CALLER: &str = "package app;\n\nimport sample.web.Filter;\n\nclass Caller {\n    String use(Filter filter) {\n        String first = filter.find(\"a\").toString();\n        assertThat(filter.find(\"b\")).isNotNull();\n        return first + filter.find(\"b\").toString() + Filter.describe().trim();\n    }\n}\n";

#[test]
fn a_call_on_a_nullable_library_result_is_reported_unless_checked_before() {
    let jar = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/jvm/fixtures/sample.jar");
    let library = Library::load(&[jar]);
    let summary = summary_of(CALLER);
    let files = [IndexedSummary {
        summary: &summary,
        in_tests: false,
    }];
    let found: Vec<String> = project_lints(&files, &library)
        .into_iter()
        .flatten()
        .filter(|finding| finding.kind.rule().as_str() == "data-flow-issue")
        .map(|finding| format!("{}:{} {}", finding.line, finding.column, finding.subject))
        .collect();
    assert_eq!(found, ["7:41 toString()"]);
}

const FLOW: &str = "package app;\n\nimport sample.web.Filter;\n\nclass Flow {\n    void unchecked(Filter filter) {\n        Object found = filter.find(\"a\");\n        found.hashCode();\n    }\n\n    void guarded(Filter filter) {\n        Object found = filter.find(\"b\");\n        if (found == null) {\n            return;\n        }\n        found.hashCode();\n        Object other = filter.find(\"c\");\n        if (other != null && other.hashCode() > 0) {\n            other.toString();\n        }\n    }\n\n    int cast(Filter filter) {\n        String text = (String) filter.find(\"d\");\n        try {\n            return text.length();\n        } finally {\n            text.trim();\n        }\n    }\n}\n";

#[test]
fn a_variable_from_a_nullable_call_is_reported_where_no_path_checks_it() {
    let jar = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/jvm/fixtures/sample.jar");
    let library = Library::load(&[jar]);
    let summary = summary_of(FLOW);
    let files = [IndexedSummary {
        summary: &summary,
        in_tests: false,
    }];
    let mut found: Vec<String> = project_lints(&files, &library)
        .into_iter()
        .flatten()
        .filter(|finding| finding.kind.rule().as_str() == "data-flow-issue")
        .map(|finding| format!("{}:{} {}", finding.line, finding.column, finding.subject))
        .collect();
    found.sort();
    assert_eq!(found, ["26:25 length()", "28:18 trim()", "8:15 hashCode()"]);
}

const JOBS: &str = "package app;\n\nimport java.io.InputStream;\nimport java.util.concurrent.ExecutorService;\nimport java.util.concurrent.Executors;\n\nclass Jobs {\n    void leak() {\n        ExecutorService pool = Executors.newFixedThreadPool(2);\n        pool.submit(() -> { });\n    }\n\n    void closed() {\n        try (ExecutorService pool = Executors.newFixedThreadPool(2)) {\n            pool.submit(() -> { });\n        }\n    }\n\n    void handed() {\n        ExecutorService pool = Executors.newFixedThreadPool(2);\n        register(pool);\n    }\n\n    void register(ExecutorService pool) {\n        pool.shutdown();\n    }\n\n    InputStream open() {\n        return null;\n    }\n\n    int read() throws Exception {\n        return open().read();\n    }\n}\n";

#[test]
fn a_resource_created_outside_try_with_resources_is_reported() {
    let summary = summary_of(JOBS);
    let files = [IndexedSummary {
        summary: &summary,
        in_tests: false,
    }];
    let found: Vec<String> = project_lints(&files, &Library::load(&[]))
        .into_iter()
        .flatten()
        .filter(|finding| finding.kind.rule().as_str() == "auto-closeable-resource")
        .map(|finding| format!("{}:{} {}", finding.line, finding.column, finding.subject))
        .collect();
    assert_eq!(found, ["9:42 ExecutorService", "33:16 InputStream"]);
}

const APPLICATION: &str = "package app;\n\n@SpringBootApplication\nclass App {\n    public static void main(String[] args) {\n    }\n}\n";
const FIELDS: &str = "package app;\n\n@Service\n@RequiredArgsConstructor\nclass Invitations {\n    private static final long serialVersionUID = 1L;\n    private static final int MAX_PAGE = 50;\n    private static final int LIMIT = 5;\n    public static final String PAID = \"PAID\";\n    private final Mailer mailer;\n    private final Clock clock;\n    @Autowired\n    private Auditor auditor;\n    @Mock\n    private Payments payments;\n\n    int send() {\n        return this.clock.hashCode() + LIMIT;\n    }\n}\n\n@Data\nclass Invitation {\n    private String email;\n}\n\n@Entity\nclass Stored {\n    @Column\n    private String key;\n\n    @interface Marked {\n        String DEFAULT = \"none\";\n    }\n}\n";

fn unused_fields(sources: &[&str]) -> Vec<String> {
    let summaries: Vec<JavaSummary> = sources.iter().map(|source| summary_of(source)).collect();
    let files: Vec<IndexedSummary<'_>> = summaries
        .iter()
        .map(|summary| IndexedSummary {
            summary,
            in_tests: false,
        })
        .collect();
    project_lints(&files, &Library::load(&[]))
        .into_iter()
        .flatten()
        .filter(|finding| finding.kind.rule().as_str() == "unused-field")
        .map(|finding| format!("{} {}", finding.line, finding.subject))
        .collect()
}

#[test]
fn a_field_nobody_names_is_reported_unless_a_framework_uses_it() {
    assert_eq!(
        unused_fields(&[APPLICATION, FIELDS]),
        ["7 MAX_PAGE", "9 PAID", "10 mailer", "13 auditor", "33 DEFAULT"]
    );
    assert_eq!(
        unused_fields(&[FIELDS]),
        ["7 MAX_PAGE", "10 mailer", "13 auditor"],
        "en una librería lo público es su API"
    );
}
