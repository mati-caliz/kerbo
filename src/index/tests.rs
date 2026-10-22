use std::path::Path;

use tree_sitter::Parser;

use super::java::{JavaSummary, summarize};
use super::{IndexedSummary, project_lints};
use crate::jvm::library::Library;
use crate::language::Language;

const FILTER: &str = "package app;\n\nimport java.util.List;\nimport sample.web.Filter;\n\nclass AuditFilter extends Filter {\n    @Override\n    protected void handle(String request, Object handler, List<String> names) {\n        if (names.isEmpty()) {\n            return;\n        }\n    }\n\n    @Override\n    public int compareTo(Filter other) {\n        return 0;\n    }\n\n    @Override\n    public String name() {\n        return \"audit\";\n    }\n\n    @Override\n    public Object find(String key) {\n        return null;\n    }\n}\n";

pub(super) fn summary_of(source: &str) -> JavaSummary {
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
    assert_eq!(unused_fields(&[APPLICATION, TOP_LEVEL_ANNOTATION]), ["4 NONE"]);
}

const TOP_LEVEL_ANNOTATION: &str = "package app;\n\n@interface Top {\n    String NONE = \"\";\n}\n";

const PRICING: &str = "package app;\n\ninterface Pricing {\n    int rate();\n    int fee();\n}\n\ninterface Flag {\n    boolean on();\n}\n\nclass Retail implements Pricing, Flag {\n    @Override public int rate() { return 4; }\n    @Override public int fee() { return 1; }\n    @Override public boolean on() { return true; }\n}\n\nclass Wholesale implements Pricing {\n    @Override public int rate() { return 4; }\n    @Override public int fee() { return 2; }\n    Pricing promo() {\n        return new Pricing() {\n            @Override public int rate() { return 4; }\n            @Override public int fee() { return 3; }\n        };\n    }\n}\n";

#[test]
fn an_abstract_method_whose_implementations_all_return_the_same_is_reported() {
    let summaries = [summary_of(APPLICATION), summary_of(PRICING)];
    let files: Vec<IndexedSummary<'_>> = summaries
        .iter()
        .map(|summary| IndexedSummary {
            summary,
            in_tests: false,
        })
        .collect();
    let found: Vec<String> = project_lints(&files, &Library::load(&[]))
        .into_iter()
        .flatten()
        .filter(|finding| finding.kind.rule().as_str() == "same-return-value")
        .map(|finding| format!("{} {}", finding.line, finding.subject))
        .collect();
    assert_eq!(found, ["4 rate() = 4"]);
}

const UNREAD: &str = "package app;\n\nenum Kind {\n    A(1, true);\n\n    private final int code;\n    private final boolean adjustment;\n    private int total;\n    private boolean seen;\n\n    Kind(int code, boolean adjustment) {\n        this.code = code;\n        this.adjustment = adjustment;\n        total += code;\n        Runnable check = new Runnable() {\n            public void run() {\n                if (seen) {\n                    return;\n                }\n                seen = true;\n            }\n        };\n    }\n\n    int code() {\n        return code;\n    }\n}\n";

#[test]
fn a_private_field_that_is_only_written_is_reported() {
    let summary = summary_of(UNREAD);
    let files = [IndexedSummary {
        summary: &summary,
        in_tests: false,
    }];
    let found: Vec<String> = project_lints(&files, &Library::load(&[]))
        .into_iter()
        .flatten()
        .filter(|finding| finding.kind.rule().as_str() == "unread-field")
        .map(|finding| format!("{} {}", finding.line, finding.subject))
        .collect();
    assert_eq!(found, ["7 adjustment"]);
}

const RESOURCES: &str = "package app;\n\nimport java.io.File;\nimport java.io.InputStream;\n\nclass Loader {\n    byte[] read() throws Exception {\n        try (InputStream stream = getClass().getResourceAsStream(\"/data\")) {\n            return stream.readAllBytes();\n        }\n    }\n\n    String version(File file) {\n        file.getParentFile().mkdirs();\n        return System.getProperty(\"java.version\").trim() + file.getName().trim();\n    }\n}\n";

#[test]
fn what_the_jdk_may_return_as_null_is_known_without_a_classpath() {
    let summary = summary_of(RESOURCES);
    let files = [IndexedSummary {
        summary: &summary,
        in_tests: false,
    }];
    let mut found: Vec<String> = project_lints(&files, &Library::load(&[]))
        .into_iter()
        .flatten()
        .filter(|finding| finding.kind.rule().as_str() == "data-flow-issue")
        .map(|finding| format!("{} {}", finding.line, finding.subject))
        .collect();
    found.sort();
    assert_eq!(found, ["14 mkdirs()", "15 trim()", "9 readAllBytes()"]);
}

const UNBOXED: &str = "package app;\n\nimport sample.web.Filter;\n\nclass Counter {\n    long count(Filter filter) {\n        return filter.find(\"a\");\n    }\n\n    Object kept(Filter filter) {\n        return filter.find(\"b\");\n    }\n\n    void local(Filter filter) {\n        int size = filter.find(\"c\");\n    }\n}\n";

#[test]
fn unboxing_a_nullable_library_result_is_reported() {
    let jar = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/jvm/fixtures/sample.jar");
    let library = Library::load(&[jar]);
    let summary = summary_of(UNBOXED);
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
    assert_eq!(found, ["7:16 find()", "15:20 find()"]);
}
