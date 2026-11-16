use std::path::Path;

use super::java_null_tests::findings_in;
use super::java_throws_tests::fixture_jdk;
use super::tests::summary_of;
use super::{IndexedSummary, project_lints};
use crate::jvm::library::Library;

const MEMBER_ROOTS: &str = r"package app;

import org.jspecify.annotations.Nullable;
import sample.fields.Rule;

public class Jenkins {
    @Nullable View getView(String name) {
        return null;
    }
}

final class View {
    int size() {
        return 0;
    }
}

final class JenkinsRule {
    Jenkins jenkins;
}

final class ViewTest {
    int count(JenkinsRule rule) {
        return rule.jenkins.getView(rule.toString()).size();
    }

    int packaged(Rule rule) {
        return rule.jenkins.getView(rule.toString()).size();
    }

    int unknown(Rule rule) {
        return rule.missing.getView(rule.toString()).size();
    }
}
";

// Una cadena que empieza en el campo de una variable (`j.jenkins.getView(...)`) toma el tipo del
// campo. Sin la librería, el campo de `Rule` no se conoce y la cadena no sigue.
#[test]
fn a_chain_rooted_at_a_field_of_a_variable_uses_the_field_type() {
    assert_eq!(findings_in(MEMBER_ROOTS, "data-flow-issue"), ["24:54 size()"]);
}

// El campo de una clase de un jar sale de su `.class`: `sample.fields.Rule` declara
// `public app.Jenkins jenkins`.
#[test]
fn the_field_of_a_library_class_comes_from_its_class_file() {
    let jar = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/jvm/fixtures/fields.jar");
    let summary = summary_of(MEMBER_ROOTS);
    let files = [IndexedSummary {
        summary: &summary,
        in_tests: false,
    }];
    let library = Library::load(&[jar]).with_jdk(fixture_jdk());
    let found: Vec<String> = project_lints(&files, &library)
        .into_iter()
        .flatten()
        .filter(|finding| finding.kind.rule().as_str() == "data-flow-issue")
        .map(|finding| format!("{}:{} {}", finding.line, finding.column, finding.subject))
        .collect();
    assert_eq!(found, ["24:54 size()", "28:54 size()"]);
}
