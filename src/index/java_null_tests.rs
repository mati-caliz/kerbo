use super::java_throws_tests::fixture_jdk;
use super::tests::summary_of;
use super::{IndexedSummary, project_lints};
use crate::jvm::library::Library;

const UTILITIES: &str = r#"package app;

import java.io.InputStream;

final class Fields {
    static Fields find(String name) {
        if (name.isEmpty()) {
            return null;
        }
        return new Fields();
    }

    static Fields always() {
        return new Fields();
    }

    Fields maybe() {
        return null;
    }

    InputStream stream() {
        return null;
    }

    static int length(String text) {
        return text.length();
    }

    static int lengths(String... texts) {
        return texts.length;
    }

    static String[] between(String text, String open) {
        if (text == null || open.isEmpty()) {
            return null;
        }
        if (text.contains(open)) {
            return null;
        }
        return new String[0];
    }

    static String[] all(String... texts) {
        if (texts == null) {
            return null;
        }
        return texts;
    }

    static Fields accessible(Fields method) {
        return method != null ? method.maybe() : null;
    }

    static int tolerant(String text) {
        if (text == null) {
            return 0;
        }
        return text.length();
    }
}

class Open {
    Open maybe() {
        return null;
    }
}

class Uses {
    void calls(Open open) {
        Fields.find("a").hashCode();
        Fields.always().hashCode();
        Fields.always().maybe().hashCode();
        open.maybe().hashCode();
        Fields.length(null);
        Fields.length((String) null);
        Fields.lengths((String[]) null);
        Fields.lengths("a", null);
        Fields.tolerant(null);
        Fields.between("a", "b").clone();
        Fields.all("a").clone();
        Fields.accessible(new Fields()).hashCode();
        System.getProperty("java.version").trim();
        System.getProperty("app.mode").trim();
    }
}
"#;

fn findings(rule: &str) -> Vec<String> {
    let summary = summary_of(UTILITIES);
    let files = [IndexedSummary {
        summary: &summary,
        in_tests: false,
    }];
    let library = Library::load(&[]).with_jdk(fixture_jdk());
    project_lints(&files, &library)
        .into_iter()
        .flatten()
        .filter(|finding| finding.kind.rule().as_str() == rule)
        .map(|finding| format!("{}:{} {}", finding.line, finding.column, finding.subject))
        .collect()
}

// `find` devuelve null y es estático: se marca. `maybe` de una clase final también; el de `Open`,
// que se puede redefinir, no.
#[test]
fn a_project_method_that_returns_null_and_cannot_be_overridden_is_nullable() {
    assert_eq!(
        findings("data-flow-issue"),
        ["70:26 hashCode()", "72:33 hashCode()", "83:40 trim()"]
    );
}

#[test]
fn null_passed_to_a_parameter_the_method_dereferences_is_reported() {
    assert_eq!(
        findings("null-argument"),
        ["74:23 text", "75:32 text", "76:35 texts"]
    );
}
