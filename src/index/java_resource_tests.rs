use super::tests::summary_of;
use super::{IndexedSummary, project_lints};
use crate::jvm::library::Library;

const SOURCES: &str = r"package app;

import static app.Streams.open;

import java.io.Closeable;
import java.io.InputStream;
import java.util.function.Supplier;

final class Streams {
    static InputStream open() {
        throw new UnsupportedOperationException();
    }

    static Tracked track() {
        throw new UnsupportedOperationException();
    }

    static void assertThrows(Class<?> type, Runnable runnable) {
        runnable.run();
    }
}

class Tracked implements Closeable {
    public void close() {
    }

    boolean isClosed() {
        return false;
    }

    int read() {
        return 0;
    }
}

final class Uses {
    void imported() {
        InputStream stream = open();
        stream.hashCode();
    }

    void projectClass() {
        Streams.track().read();
    }

    void checkedLater() {
        Tracked tracked = new Tracked();
        tracked.isClosed();
    }

    void inLambda() {
        Streams.assertThrows(RuntimeException.class, () -> Streams.track());
    }
}
";

// Una llamada traída con `import static` y una que devuelve una clase del proyecto que se cierra
// crean recursos; una llamada con "close" en el nombre en el mismo bloque da la local por escapada;
// la lambda de `assertThrows` que sólo crea un recurso lo pierde.
#[test]
fn imported_calls_project_resources_and_closing_names_follow_intellij() {
    let summary = summary_of(SOURCES);
    let files = [IndexedSummary {
        summary: &summary,
        in_tests: false,
    }];
    let mut found: Vec<String> = project_lints(&files, &Library::load(&[]))
        .into_iter()
        .flatten()
        .filter(|finding| finding.kind.rule().as_str() == "auto-closeable-resource")
        .map(|finding| format!("{}:{} {}", finding.line, finding.column, finding.subject))
        .collect();
    found.sort();
    assert_eq!(found, ["38:30 InputStream", "43:17 Tracked", "52:68 Tracked"]);
}

const BUILDERS: &str = r#"package app;

import java.io.Closeable;

class Client implements Closeable {
    public void close() {
    }

    Client login(String user) {
        return this;
    }

    Client withRedirects(boolean enabled) {
        return this;
    }

    int read() {
        return 0;
    }
}

final class Rule {
    Client createClient() {
        throw new UnsupportedOperationException();
    }
}

final class Recorder implements Closeable {
    public void close() {
    }

    Recorder record(String name) {
        return this;
    }
}

final class Uses {
    private final Rule rule = new Rule();
    private final Recorder logging = new Recorder();

    void inTry() {
        try (Client client = rule.createClient().login("alice").withRedirects(false)) {
            client.read();
        }
    }

    void lost() {
        rule.createClient().login("bob").read();
    }

    void kept() {
        Client client = rule.createClient().login("carol");
        client.read();
    }

    void onField() {
        logging.record("a").record("b");
    }

    void inLambda(java.util.List<Client> clients) {
        Runnable task = () -> {
            Client client = rule.createClient();
            clients.add(client);
        };
        task.run();
    }

    Client returned() {
        return rule.createClient().login("dave");
    }
}
"#;

// Las llamadas que devuelven el mismo tipo (un builder) llevan lo creado a la variable de la de más
// afuera: en un `try (...)` no se marca; en una local que no se cierra, sí, en la creación. Un
// `return` al final de la cadena no la salva. Una local declarada en una lambda escapa por lo que
// pasa en esa lambda. Un receptor
// que se cierra, aunque sea un campo, es de otro.
#[test]
fn builder_chains_follow_the_outermost_call_and_closeable_receivers_own_the_result() {
    let summary = summary_of(BUILDERS);
    let files = [IndexedSummary {
        summary: &summary,
        in_tests: false,
    }];
    let mut found: Vec<String> = project_lints(&files, &Library::load(&[]))
        .into_iter()
        .flatten()
        .filter(|finding| finding.kind.rule().as_str() == "auto-closeable-resource")
        .map(|finding| format!("{}:{} {}", finding.line, finding.column, finding.subject))
        .collect();
    found.sort();
    assert_eq!(found, ["48:14 Client", "52:30 Client", "69:21 Client"]);
}
