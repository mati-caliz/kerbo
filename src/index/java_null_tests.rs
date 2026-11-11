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
        Lookup.pick("a", "b").hashCode();
        final Fields field = new Fields();
        Lookup.typed(field).hashCode();
        Lookup.typed((Object) field).hashCode();
        Lookup.copied("a").hashCode();
        final Fields found = Fields.find("b");
        Lookup.code(found);
        found.hashCode();
        final Fields other = Fields.find("c");
        Lookup.typed(other);
        other.hashCode();
        Lookup.unrolled(null);
    }
}

final class Lookup {
    static Lookup none(String... names) {
        return null;
    }

    static Lookup pick(String name, Object value) {
        return null;
    }

    static Lookup pick(String name, Object... values) {
        return new Lookup();
    }

    static Lookup typed(Fields field) {
        return null;
    }

    static Lookup typed(Object value) {
        return new Lookup();
    }

    static Lookup copied(String name) {
        final String copy = name == null ? "" : name;
        if (copy.isEmpty()) {
            return null;
        }
        return new Lookup();
    }

    static int code(Fields field) {
        return field.hashCode();
    }

    static int unrolled(String text) {
        int size;
        do {
            size = text.length();
            if (size > 0) {
                break;
            }
        } while (true);
        return size;
    }
}
"#;

const OVERLOADS: &str = r"package app;

import java.util.Collection;
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
    void calls(Counter shared, Future<String> pending, Collection<String> items) throws Exception {
        final Counter counter = new Counter(0);
        new Counter((Number) null);
        new Counter((String) null);
        counter.setValue(null);
        counter.compareTo(null);
        shared.compareTo(null);
        Counter.total((int[][]) null);
        pending.get(1, null);
        pending.get(1, TimeUnit.SECONDS);
        items.removeAll(null);
        items.add(null);
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
            "85:23 hashCode()",
            "86:31 hashCode()",
            "88:29 hashCode()",
            "90:28 hashCode()",
            "96:15 hashCode()"
        ]
    );
}

#[test]
fn null_passed_to_a_parameter_the_method_dereferences_is_reported() {
    assert_eq!(
        findings("null-argument"),
        [
            "74:23 text",
            "75:32 text",
            "76:35 texts",
            "97:25 text",
            "92:21 field"
        ]
    );
}

// Una sobrecarga con un primitivo no recibe el `null` y un casteo elige la del tipo; con un
// receptor recién creado, IntelliJ resuelve el método aunque se pueda redefinir. Del JDK, lo que
// sus anotaciones externas dan por no nulo.
#[test]
fn null_reaches_only_the_overloads_that_can_receive_it() {
    assert_eq!(
        findings_in(OVERLOADS, "null-argument"),
        [
            "46:30 value",
            "48:26 value",
            "49:27 other",
            "51:33 groups",
            "52:24 unit",
            "54:25 c"
        ]
    );
}

const PASSED: &str = r#"package app;

import java.net.URL;

class Paths {
    void parents(Class<?> type) {
        expect(type.getResource("a"));
        type.getResource("a").getPath();
        tolerate(type.getResource("b"));
        type.getResource("b").getPath();
    }

    void together(Class<?> type) {
        pair(type.getResource("c"), type.getResource("c").getPath());
    }

    private void expect(URL url) {
        url.getPath();
    }

    private void tolerate(URL url) {
        if (url == null) {
            return;
        }
    }

    private void pair(URL url, String path) {
        url.getPath();
    }

    void twice(Paths other) {
        other.maybe().hashCode();
        other.maybe().hashCode();
        other.maybe("a").hashCode();
        other.maybe("a").hashCode();
        Paths again = other;
        again.maybe().hashCode();
        again = this;
        again.maybe().hashCode();
        make("a").maybe().hashCode();
        make("a").maybe().hashCode();
    }

    private static Paths make(String name) {
        return new Paths();
    }

    private Paths maybe() {
        return null;
    }

    private Paths maybe(String name) {
        return null;
    }
}
"#;

// Pasar el resultado a un parámetro que el método desreferencia lo da por no nulo después: no
// adentro de la misma llamada ni si el método lo tolera null. Un getter sin argumentos ya
// desreferenciado tampoco vuelve a avisar, salvo que se reasigne la variable o que la cadena
// tenga argumentos.
#[test]
fn a_call_passed_to_a_non_null_parameter_is_not_null_afterwards() {
    assert_eq!(
        findings_in(PASSED, "data-flow-issue"),
        [
            "10:31 getPath()",
            "14:59 getPath()",
            "32:23 hashCode()",
            "34:26 hashCode()",
            "35:26 hashCode()",
            "37:23 hashCode()",
            "39:23 hashCode()",
            "40:27 hashCode()",
            "41:27 hashCode()"
        ]
    );
}
