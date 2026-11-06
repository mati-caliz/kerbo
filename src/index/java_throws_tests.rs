use std::path::Path;

use super::tests::summary_of;
use super::{IndexedSummary, project_lints};
use crate::jvm::jdk::Jdk;
use crate::jvm::library::Library;

const JAVA_17: u32 = 17;
const SERVICE: &str = r#"package app;

import java.io.FileInputStream;
import java.io.FileNotFoundException;
import java.io.IOException;

class Service {
    void opens() throws IOException {
        try (FileInputStream input = new FileInputStream("a")) {
            input.read();
        }
    }

    void neverThrows(String text) throws IOException {
        text.trim();
    }

    void general() throws Exception {
        helper();
    }

    void helper() {
    }

    void handled() throws IOException {
        try {
            new FileInputStream("a");
        } catch (IOException problem) {
            helper();
        }
    }

    void unchecked() throws RuntimeException {
    }

    void inLambda() throws IOException {
        Object later = () -> new FileInputStream("a");
    }

    void unresolved() throws IOException {
        missing.call();
    }

    void narrower() throws IOException {
        throw new FileNotFoundException("a");
    }

    void rethrown() throws IOException {
        try {
            opens();
        } catch (IOException problem) {
            throw problem;
        }
    }

    void writeObject(Object stream) throws IOException {
    }

    void caughtByCaller() throws FileNotFoundException {
    }

    void catches() {
        try {
            caughtByCaller();
        } catch (FileNotFoundException problem) {
            helper();
        }
    }

    Service() throws IOException {
    }
}

abstract class Base {
    abstract void abstracted() throws IOException;

    void overridden() throws IOException {
    }
}

class Child extends Base {
    @Override
    void abstracted() throws IOException {
        throw new IOException();
    }

    @Override
    void overridden() throws IOException {
        throw new IOException();
    }
}

class Anonymous {
    Object first = new Base() {
        void abstracted() throws IOException {
        }
    };

    Object second = new Base() {
        void abstracted() throws IOException {
        }
    };

    void throwsOnlyUnchecked() throws Exception {
        throw new RuntimeException();
    }

    void readsTheCaughtVariable() throws IOException {
        try {
            new FileInputStream("a");
        } catch (IOException problem) {
            problem.getMessage();
        }
    }
}
"#;

fn fixture_jdk() -> Option<Jdk> {
    Jdk::at(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src/jvm/fixtures/jdk"),
        Some(JAVA_17),
    )
}

fn redundant_with(library: &Library) -> Vec<String> {
    let summary = summary_of(SERVICE);
    let files = [IndexedSummary {
        summary: &summary,
        in_tests: false,
    }];
    project_lints(&files, library)
        .into_iter()
        .flatten()
        .filter(|finding| finding.kind.rule().as_str() == "redundant-throws")
        .map(|finding| format!("{}:{} {}", finding.line, finding.column, finding.subject))
        .collect()
}

#[test]
fn a_throws_nothing_can_reach_is_reported_and_anything_unknown_keeps_quiet() {
    let library = Library::load(&[]).with_jdk(fixture_jdk());
    let mut found = redundant_with(&library);
    found.sort();
    assert_eq!(
        found,
        [
            "100:34 IOException",
            "104:39 Exception",
            "108:42 IOException",
            "14:42 IOException",
            "18:27 Exception",
            "25:27 IOException",
            "36:28 IOException",
            "95:34 IOException",
        ]
    );
}

#[test]
fn without_a_jdk_nothing_can_be_resolved_and_nothing_is_reported() {
    assert_eq!(redundant_with(&Library::load(&[])), Vec::<String>::new());
}
