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
