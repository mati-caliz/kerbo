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

class Built {
    Built() throws IOException {
    }

    Built(int size) throws IOException {
        this();
    }
}

class Derived extends FileInputStream {
    Derived() throws IOException {
        super("a");
    }
}

class Initialized {
    Object stream = new FileInputStream("a");

    Initialized() throws IOException {
    }
}

class Rethrows {
    void widened() throws FileNotFoundException {
        try {
            new FileInputStream("a");
        } catch (IOException problem) {
            throw problem;
        }
    }
}

class Unrelated {
    void get() throws FileNotFoundException {
    }
}

class Other {
    void get() throws FileNotFoundException {
        throw new FileNotFoundException("a");
    }

    void calls(Other other) {
        try {
            other.get();
        } catch (FileNotFoundException problem) {
            other.toString();
        }
    }
}
"#;

const CONSTANTS: &str = r#"package app;

import static app.Holder.OPENER;

import java.io.FileInputStream;
import java.io.IOException;

class Holder {
    static final Opener OPENER = new Opener();
}

class Opener {
    void open() throws IOException {
        new FileInputStream("a");
    }
}

class User {
    void imported() throws IOException {
        OPENER.open();
    }

    void qualified() throws IOException {
        Holder.OPENER.open();
    }

    void quiet() throws IOException {
        OPENER.toString();
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
    redundant_in(SERVICE, library)
}

fn redundant_in(source: &str, library: &Library) -> Vec<String> {
    let summary = summary_of(source);
    let files = [IndexedSummary {
        summary: &summary,
        in_tests: false,
    }];
    let mut found: Vec<String> = project_lints(&files, library)
        .into_iter()
        .flatten()
        .filter(|finding| finding.kind.rule().as_str() == "redundant-throws")
        .map(|finding| format!("{}:{} {}", finding.line, finding.column, finding.subject))
        .collect();
    found.sort();
    found
}

#[test]
fn a_throws_nothing_can_reach_is_reported_and_anything_unknown_keeps_quiet() {
    let library = Library::load(&[]).with_jdk(fixture_jdk());
    assert_eq!(
        redundant_with(&library),
        [
            "100:34 IOException",
            "104:39 Exception",
            "108:42 IOException",
            "118:20 IOException",
            "14:42 IOException",
            "150:23 FileNotFoundException",
            "18:27 Exception",
            "25:27 IOException",
            "36:28 IOException",
            "70:22 IOException",
            "95:34 IOException",
        ]
    );
}

#[test]
fn a_constant_is_followed_by_its_type_whether_imported_or_qualified() {
    let library = Library::load(&[]).with_jdk(fixture_jdk());
    assert_eq!(redundant_in(CONSTANTS, &library), ["27:25 IOException"]);
}

#[test]
fn without_a_jdk_nothing_can_be_resolved_and_nothing_is_reported() {
    assert_eq!(redundant_with(&Library::load(&[])), Vec::<String>::new());
}
