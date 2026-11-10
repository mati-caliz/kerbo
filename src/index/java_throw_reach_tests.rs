use super::java_throws_tests::{fixture_jdk, redundant_in};
use crate::jvm::library::Library;

const ROOTS: &str = r"package app;

import java.io.FileInputStream;
import java.io.IOException;

class Roots {
    void cast(Object opened) throws IOException {
        ((FileInputStream) opened).close();
    }

    void array(FileInputStream[] opened) throws IOException {
        opened[0].close();
    }

    void castQuiet(Object opened) throws IOException {
        ((FileInputStream) opened).hashCode();
    }
}
";

const OVERRIDERS: &str = r#"package app;

import java.io.FileInputStream;
import java.io.IOException;

class Named {
    void run() throws IOException {
    }
}

class Child extends Named {
    void run() throws IOException {
        new FileInputStream("a");
    }
}

class Anonymous {
    void run() throws IOException {
    }

    static Anonymous create() {
        return new Anonymous() {
            void run() throws IOException {
                new FileInputStream("a");
            }
        };
    }
}

class Declares {
    void run() throws IOException {
    }
}

class OnlyDeclares extends Declares {
    void run() throws IOException {
    }
}
"#;

const ANONYMOUS_CREATIONS: &str = r"package app;

import java.io.IOException;

class Plain {
}

class Made {
    Made() {
    }
}

interface Task {
    void go();
}

class Spawns {
    void plain() throws IOException {
        new Plain() { };
    }

    void made() throws IOException {
        new Made() { };
    }

    void interfaced() throws IOException {
        new Runnable() {
            public void run() {
            }
        };
    }

    void tasked() throws IOException {
        new Task() {
            public void go() {
            }
        };
    }
}
";

const LAMBDAS: &str = r"package app;

import java.io.IOException;
import java.util.Map;

class Lazy {
    void open() throws IOException {
    }
}

class Eager {
    void open() throws IOException {
    }
}

class Users {
    void each(Map<String, Lazy> all) {
        all.forEach((name, lazy) -> {
            try {
                lazy.open();
            } catch (IOException problem) {
                name.trim();
            }
        });
    }
}
";

const GENERICS: &str = r"package app;

import java.io.IOException;

interface Source<T, E extends Exception> {
    T get() throws E;
}

interface Opening<T> extends Source<T, IOException> {
}

interface Hiding<E extends Exception> {
    <E extends Exception> void run() throws E;
}

class Late extends Exception {
}

class Text {
    void show() {
    }
}

class Users {
    void bound(Opening<String> opening) throws IOException, Late {
        opening.get();
    }

    void direct(Source<Text, IOException> source) throws IOException, Late {
        source.get().show();
    }

    void raw(Opening opening) throws Exception, Late {
        opening.get();
    }

    void hidden(Hiding<IOException> hiding) throws Exception, Late {
        hiding.run();
    }
}
";

#[test]
fn casts_parentheses_and_array_elements_are_followed_by_their_types() {
    let library = Library::load(&[]).with_jdk(fixture_jdk());
    assert_eq!(redundant_in(ROOTS, &library), ["15:42 IOException"]);
}

#[test]
fn only_what_the_body_of_an_overrider_throws_keeps_the_throws_of_its_parent() {
    let library = Library::load(&[]).with_jdk(fixture_jdk());
    assert_eq!(
        redundant_in(OVERRIDERS, &library),
        ["31:23 IOException", "36:23 IOException"]
    );
}

#[test]
fn an_anonymous_class_of_an_interface_keeps_the_method_quiet() {
    let library = Library::load(&[]).with_jdk(fixture_jdk());
    assert_eq!(
        redundant_in(ANONYMOUS_CREATIONS, &library),
        ["18:25 IOException", "22:24 IOException"]
    );
}

#[test]
fn a_call_on_a_for_each_parameter_only_excuses_the_type_of_the_elements() {
    let library = Library::load(&[]).with_jdk(fixture_jdk());
    assert_eq!(redundant_in(LAMBDAS, &library), ["12:24 IOException"]);
}

#[test]
fn the_type_arguments_of_a_project_type_say_what_its_inherited_throws_throw() {
    let library = Library::load(&[]).with_jdk(fixture_jdk());
    assert_eq!(redundant_in(GENERICS, &library), ["25:61 Late", "29:71 Late"]);
}
