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
        Lookup.none("a", "b").hashCode();
        Lookup.none().hashCode();
    }
}

final class Lookup {
    static Lookup none(String... names) {
        return null;
    }
}
"#;

const OVERLOADS: &str = r"package app;

import java.util.concurrent.Future;
import java.util.concurrent.TimeUnit;

class Counter {
    private int value;

    Counter(int value) {
        this.value = value;
    }

    Counter(Number value) {
        this.value = value.intValue();
    }

    Counter(String value) {
        this.value = Integer.parseInt(value);
    }

    void setValue(int value) {
        this.value = value;
    }

    void setValue(Number value) {
        this.value = value.intValue();
    }

    int compareTo(Counter other) {
        return Integer.compare(this.value, other.value);
    }

    static int total(int[]... groups) {
        int total = 0;
        for (int[] group : groups) {
            total += group.length;
        }
        return total;
    }
}

class Uses {
    void calls(Counter shared, Future<String> pending) throws Exception {
        final Counter counter = new Counter(0);
        new Counter((Number) null);
        new Counter((String) null);
        counter.setValue(null);
        counter.compareTo(null);
        shared.compareTo(null);
        Counter.total((int[][]) null);
        pending.get(1, null);
        pending.get(1, TimeUnit.SECONDS);
    }

    void other() {
        Counter counter = new Counter(1);
        counter = new Counter(2);
        counter.compareTo(null);
    }
}
";

fn findings(rule: &str) -> Vec<String> {
    findings_in(UTILITIES, rule)
}

fn findings_in(source: &str, rule: &str) -> Vec<String> {
    let summary = summary_of(source);
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
        [
            "70:26 hashCode()",
            "72:33 hashCode()",
            "83:40 trim()",
            "84:31 hashCode()",
            "85:23 hashCode()"
        ]
    );
}

#[test]
fn null_passed_to_a_parameter_the_method_dereferences_is_reported() {
    assert_eq!(
        findings("null-argument"),
        ["74:23 text", "75:32 text", "76:35 texts"]
    );
}

// Una sobrecarga con un primitivo no recibe el `null` y un casteo elige la del tipo; con un
// receptor recién creado, IntelliJ resuelve el método aunque se pueda redefinir.
#[test]
fn null_reaches_only_the_overloads_that_can_receive_it() {
    assert_eq!(
        findings_in(OVERLOADS, "null-argument"),
        [
            "45:30 value",
            "47:26 value",
            "48:27 other",
            "50:33 groups",
            "51:24 unit"
        ]
    );
}
