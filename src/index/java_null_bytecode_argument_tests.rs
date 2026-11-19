use std::path::Path;

use super::java_hardcoded_contracts::has_hardcoded_contract;
use super::java_throws_tests::fixture_jdk;
use super::tests::summary_of;
use super::{IndexedSummary, project_lints};
use crate::jvm::library::Library;

fn findings_with_the_jar(source: &str, rule: &str) -> Vec<String> {
    let jar = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/jvm/fixtures/inferred.jar");
    let summary = summary_of(source);
    let files = [IndexedSummary {
        summary: &summary,
        in_tests: false,
    }];
    let library = Library::load(&[jar]).with_jdk(fixture_jdk());
    project_lints(&files, &library)
        .into_iter()
        .flatten()
        .filter(|finding| finding.kind.rule().as_str() == rule)
        .map(|finding| format!("{}:{} {}", finding.line, finding.column, finding.subject))
        .collect()
}

const CALLS: &str = r"package app;

import org.jspecify.annotations.Nullable;
import sample.inferred.Checks;
import sample.inferred.Overloads;

final class Calls {
    static @Nullable String first() {
        return null;
    }

    static @Nullable String second() {
        return null;
    }

    static @Nullable String third() {
        return null;
    }

    static @Nullable String fourth() {
        return null;
    }

    static @Nullable String fifth() {
        return null;
    }

    static @Nullable String sixth() {
        return null;
    }

    static @Nullable String seventh() {
        return null;
    }

    void call() {
        Checks.direct(first());
        Checks.untouched(second());
        Checks.forwarded(third());
        new Checks(fourth());
        Overloads.size(fifth());
        Overloads.exported(fifth());
        Overloads.all(sixth());
        Overloads.any(seventh());
        Overloads.any(null);
        Overloads.size(null);
    }
}
";

// Lo que IntelliJ infiere del bytecode de un jar: `direct` desreferencia su parámetro, `forwarded`
// se lo pasa a `direct` y el constructor también lo usa; `untouched` lo devuelve tal cual. La
// sobrecarga sale del tipo de lo que se pasa: un `String` va a `size(String)` y a `all(String)`,
// no a `size(StringBuilder)` ni a la de varargs. Un `null` escrito no tiene tipo: en `any` corre
// la fija porque recibe un `Object`, y en `size` las dos públicas no coinciden.
#[test]
fn a_nullable_call_passed_to_a_parameter_the_bytecode_dereferences_is_reported() {
    let found = findings_with_the_jar(CALLS, "null-argument");
    assert_eq!(
        found,
        [
            "37:23 String",
            "39:26 String",
            "40:20 String",
            "41:24 String",
            "42:28 String",
            "43:23 String",
            "44:23 Object",
            "45:23 Object"
        ]
    );
}

// `requireNonNull`, los asserts de JUnit y cualquier `equals` tienen contrato propio en IntelliJ; un
// método cualquiera de esas clases, no.
#[test]
fn methods_with_a_hardcoded_contract_are_known() {
    assert!(has_hardcoded_contract("java.util.Objects", "requireNonNull", 1));
    assert!(has_hardcoded_contract("org.junit.Assert", "assertNotNull", 1));
    assert!(has_hardcoded_contract("org.hamcrest.MatcherAssert", "fail", 0));
    assert!(has_hardcoded_contract("sample.inferred.Checks", "equals", 1));
    assert!(!has_hardcoded_contract("java.util.Objects", "hash", 1));
    assert!(!has_hardcoded_contract("org.junit.Assert", "assertEquals", 2));
    assert!(!has_hardcoded_contract("sample.inferred.Checks", "equals", 2));
}

const MADE: &str = r"package app;

import org.jspecify.annotations.Nullable;
import sample.inferred.Factory;

class Made {
    @Nullable Made next() {
        return null;
    }

    void use() {
    }
}

final class Maker {
    void call(Factory factory) {
        factory.make().next().use();
        factory.then(inner -> inner.make().next().use());
    }
}
";

// Un método de un jar puede devolver una clase del proyecto (`Factory.make()` devuelve un
// `app.Made`): la cadena sigue con los métodos de esa clase. Y el parámetro de una lambda pasada a
// un método del jar toma su tipo del método abstracto que la interfaz (`Factory.Step`) hereda de
// otra.
#[test]
fn a_library_method_returning_a_project_class_keeps_the_chain_going() {
    assert_eq!(
        findings_with_the_jar(MADE, "data-flow-issue"),
        ["17:31 use()", "18:51 use()"]
    );
}
