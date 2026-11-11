use super::java_throws_tests::fixture_jdk;
use super::tests::summary_of;
use super::{IndexedSummary, project_lints};
use crate::jvm::library::Library;

const PACKAGE_INFO: &str = "@NullMarked
package app;

import org.jspecify.annotations.NullMarked;
";

const MARKED: &str = r"package app;

import org.jspecify.annotations.NullUnmarked;
import org.jspecify.annotations.Nullable;

interface Strings {
    String pad(String text, int length);

    String tolerate(@Nullable String text);

    <T> T first(T value);

    <T extends @Nullable Object> T any(T value);

    String wrap(java.util.@Nullable Optional<String> optional);
}

@NullUnmarked
class Legacy {
    void take(String text) {
    }
}

class Box<E extends @Nullable Object> {
    void put(E element) {
    }

    void name(String name) {
    }
}

class Uses {
    void calls(Strings strings, Legacy legacy, Box<String> box) {
        strings.pad(null, 1);
        strings.tolerate(null);
        strings.first(null);
        strings.any(null);
        legacy.take(null);
        box.put(null);
        box.name(null);
        strings.wrap(null);
    }
}
";

const TEXTS: &str = r"package app;

final class Texts {
    static String repeat(String text, int count) {
        return text;
    }
}
";

const IMPORTER: &str = r"package app.uses;

import static app.Texts.repeat;

final class Callers {
    void calls() {
        repeat(null, 2);
        trim(null);
    }

    void trim(String text) {
    }
}
";

fn null_arguments(sources: &[&str]) -> Vec<String> {
    let summaries: Vec<_> = sources.iter().map(|source| summary_of(source)).collect();
    let files: Vec<IndexedSummary<'_>> = summaries
        .iter()
        .map(|summary| IndexedSummary {
            summary,
            in_tests: false,
        })
        .collect();
    let library = Library::load(&[]).with_jdk(fixture_jdk());
    project_lints(&files, &library)
        .into_iter()
        .flatten()
        .filter(|finding| finding.kind.rule().as_str() == "null-argument")
        .map(|finding| format!("{}:{} {}", finding.line, finding.column, finding.subject))
        .collect()
}

// Bajo `@NullMarked` del paquete, un parámetro sin anotar es no nulo aunque el método no tenga
// cuerpo: no si dice `@Nullable`, si su tipo es una variable que admite null o si el tipo dice
// `@NullUnmarked`, ni si el tipo calificado lleva `@Nullable` en su último tramo.
#[test]
fn null_marked_parameters_are_non_null_unless_they_admit_null() {
    assert_eq!(
        null_arguments(&[PACKAGE_INFO, MARKED]),
        ["34:21 text", "36:23 value", "40:18 name"]
    );
}

#[test]
fn without_null_marked_unannotated_parameters_say_nothing() {
    assert_eq!(null_arguments(&[MARKED]), Vec::<String>::new());
}

// La llamada sin calificar a un método traído con `import static` va a la clase del import.
#[test]
fn null_passed_to_a_statically_imported_method_reaches_its_class() {
    assert_eq!(null_arguments(&[PACKAGE_INFO, TEXTS, IMPORTER]), ["7:16 text"]);
}
