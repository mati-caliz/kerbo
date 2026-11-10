use super::java_throws_tests::fixture_jdk;
use super::tests::summary_of;
use super::{IndexedSummary, project_lints};
use crate::jvm::library::Library;

const DECLARED: &str = r#"package app;

import java.io.FileInputStream;
import java.io.IOException;

abstract class Shape {
    abstract void draw() throws IOException;

    abstract void load() throws IOException;
}

class Circle extends Shape {
    void draw() {
    }

    void load() throws IOException {
        new FileInputStream("a");
    }
}

interface Pair {
    void first() throws IOException;

    void second() throws IOException;
}

interface Unused {
    void run(String text) throws IOException;
}

interface Named {
    void run(String text) throws IOException;
}

interface Passed {
    void accept(String first, String second) throws IOException;
}

interface Mismatched {
    void accept(String first, String second, String third) throws IOException;
}

interface Inherited {
    void call() throws IOException;
}

interface Narrowed extends Inherited {
}

class Taker {
    static void take(Passed passed) {
    }
}
"#;

const USERS: &str = r"package app;

class Users {
    Mismatched never = null;

    void use() {
        Taker.take((first, second) -> { });
        Object named = (Named) text -> { };
        Narrowed narrowed = () -> { };
    }
}
";

fn redundant_across(sources: &[&str], library: &Library) -> Vec<String> {
    let summaries: Vec<_> = sources.iter().map(|source| summary_of(source)).collect();
    let files: Vec<_> = summaries
        .iter()
        .map(|summary| IndexedSummary {
            summary,
            in_tests: false,
        })
        .collect();
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
fn an_abstract_method_is_reported_unless_an_implementation_or_a_fitting_lambda_may_throw() {
    let library = Library::load(&[]).with_jdk(fixture_jdk());
    assert_eq!(
        redundant_across(&[DECLARED, USERS], &library),
        [
            "22:25 IOException",
            "24:26 IOException",
            "28:34 IOException",
            "40:67 IOException",
            "7:33 IOException",
        ]
    );
}

const CONSTANTS: &str = r#"package app;

import java.io.FileInputStream;
import java.io.IOException;

enum Reader {
    FILE {
        @Override
        void read() throws IOException {
            new FileInputStream("a");
        }

        @Override
        void check() {
        }
    };

    abstract void read() throws IOException;

    abstract void check() throws IOException;
}
"#;

#[test]
fn the_body_of_an_enum_constant_overrides_the_abstract_methods_of_its_enum() {
    let library = Library::load(&[]).with_jdk(fixture_jdk());
    assert_eq!(redundant_across(&[CONSTANTS], &library), ["20:34 IOException"]);
}
