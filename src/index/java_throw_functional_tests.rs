use super::java_throws_tests::{fixture_jdk, redundant_in};
use crate::jvm::library::Library;

const FUNCTIONAL: &str = r#"package app;

import java.io.FileInputStream;
import java.io.IOException;

class Late extends Exception {
}

interface Failable<T, E extends Throwable> {
    void accept(T value) throws E;
}

class Files {
    static void open(String name) throws IOException {
        new FileInputStream(name);
    }

    static void quiet(String name) {
    }

    Files(String name) throws IOException {
        new FileInputStream(name);
    }
}

class Runner {
    static <T, E extends Throwable> void run(Failable<T, E> action, T value) throws E {
        action.accept(value);
    }

    static <E extends Throwable> void typed(Failable<String, E> action, E sample) throws E {
    }
}

class Users {
    void lambda() throws IOException, Late {
        Runner.run(name -> Files.open(name), "a");
    }

    void caught() throws IOException {
        Runner.run(name -> {
            try {
                Files.open(name);
            } catch (IOException problem) {
                problem.hashCode();
            }
        }, "a");
    }

    void reference() throws IOException, Late {
        Runner.run(Files::open, "a");
    }

    void quietReference() throws IOException {
        Runner.run(Files::quiet, "a");
    }

    void constructor() throws IOException, Late {
        Runner.run(Files::new, "a");
    }

    void sample() throws IOException {
        Runner.typed(name -> { }, new IOException());
    }
}
"#;

#[test]
fn a_generic_throws_is_what_the_lambda_or_method_reference_passed_for_it_throws() {
    let library = Library::load(&[]).with_jdk(fixture_jdk());
    assert_eq!(
        redundant_in(FUNCTIONAL, &library),
        [
            "36:39 Late",
            "40:26 IOException",
            "50:42 Late",
            "54:34 IOException",
            "58:44 Late"
        ]
    );
}

const CONTRACTS: &str = r"package app;

import java.io.IOException;

abstract class Service {
    static <X extends Exception> void throwIfInstance(Throwable thrown, Class<X> type) throws X {
    }
}

class Quiet extends Service {
    void close(Throwable thrown) throws IOException {
        throwIfInstance(thrown, IOException.class);
    }

    void extra() throws IOException {
    }
}
";

#[test]
fn a_throws_decided_by_a_class_argument_is_not_reported() {
    let library = Library::load(&[]).with_jdk(fixture_jdk());
    assert_eq!(redundant_in(CONTRACTS, &library), ["15:25 IOException"]);
}
