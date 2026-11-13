use super::java_null_tests::{findings_in, findings_in_files};

const NULLABLE_FIELDS: &str = r"package app;

import org.jspecify.annotations.Nullable;

final class Holder {
    private @Nullable Thread worker;
    private final @Nullable StringBuilder buffer = null;
    private @Nullable Object[] cells;
    private volatile @Nullable Holder next;

    void join() throws InterruptedException {
        worker.join();
        worker.join();
    }

    void checked() {
        if (worker != null) {
            worker.interrupt();
        }
        if (this.buffer == null) {
            return;
        }
        buffer.append('a');
    }

    void started() {
        worker = new Thread();
        worker.start();
    }

    void copied() {
        Holder local = this.next;
        local.hashCode();
        cells.clone();
    }

    void shadowed(Thread worker) {
        worker.start();
        this.worker.start();
    }
}
";

// Un campo `@Nullable` se marca en su primer uso sin chequeo, también copiado a una local; no
// después de chequearlo o de asignarle algo, ni si un parámetro lo tapa. Con anotaciones de tipo,
// `@Nullable Object[]` habla de los elementos.
#[test]
fn a_nullable_field_is_reported_on_its_first_unchecked_use() {
    assert_eq!(
        findings_in(NULLABLE_FIELDS, "data-flow-issue"),
        ["12:16 join()", "33:15 hashCode()", "39:21 start()"]
    );
}

const GUARDED_FIELDS: &str = r#"package app;

import org.jspecify.annotations.Nullable;
import static com.google.common.base.Preconditions.checkState;
import static java.util.Objects.requireNonNull;

final class Guarded {
    private @Nullable Guarded tail;
    private char[] @Nullable [] replacements;

    int qualifier() {
        return tail.replacements.length;
    }

    void conditions() {
        checkState(tail != null, "sin cola");
        tail.hashCode();
    }

    void eager(boolean flag) {
        if (tail != null & flag) {
            tail.hashCode();
        }
    }

    int nested() {
        return requireNonNull(tail).hashCode() + tail.hashCode();
    }

    int elements() {
        return replacements.length;
    }

    Runnable anonymous() {
        return new Runnable() {
            public void run() {
                tail.hashCode();
            }
        };
    }
}
"#;

// El campo de un campo se marca en el calificador. Lo dejan presente un `checkState` de la
// condición, un `&` sin cortocircuito y un `requireNonNull` anidado. En `char[] @Nullable []` la
// anotación habla de los elementos, y una clase anónima no ve los campos de la de afuera.
#[test]
fn a_guarded_nullable_field_is_not_reported() {
    assert_eq!(findings_in(GUARDED_FIELDS, "data-flow-issue"), ["12:16 tail()"]);
}

const QUALIFIED_FIELDS: &str = r"package app;

import org.jspecify.annotations.Nullable;

final class Segment {
    volatile @Nullable StringBuilder table;
    @Nullable Object[] cells;
    StringBuilder plain;
}

final class Reader {
    int read(Segment segment) {
        segment.plain.length();
        segment.cells.clone();
        return segment.table.length() + segment.table.length();
    }

    int copied(Segment segment) {
        StringBuilder table = segment.table;
        return table.length();
    }

    int checked(Segment segment) {
        if (segment.table == null) {
            return 0;
        }
        return segment.table.length();
    }

    void looped(Segment segment) {
        StringBuilder table;
        while ((table = segment.table) != null) {
            table.setLength(0);
        }
    }
}
";

// El campo `@Nullable` de otra clase del proyecto, leído desde una variable de ese tipo, se marca en
// su primer uso sin chequeo, también copiado a una local; no si se chequea al asignarlo.
#[test]
fn a_nullable_field_of_another_object_is_reported() {
    assert_eq!(
        findings_in(QUALIFIED_FIELDS, "data-flow-issue"),
        ["15:30 length()", "20:22 length()"]
    );
}

const NULLABLE_RETURNS: &str = r"package app;

import javax.annotation.CheckForNull;
import org.jspecify.annotations.Nullable;

interface Lookup {
    @Nullable StringBuilder find();

    @CheckForNull StringBuilder checked();

    @Nullable Object[] cells();

    Object @Nullable [] maybeCells();
}

final class Client {
    int use(Lookup lookup) {
        lookup.cells().clone();
        lookup.maybeCells().clone();
        return lookup.find().length() + lookup.checked().length();
    }
}
";

// Un método del proyecto con el retorno `@Nullable` o `@CheckForNull` se marca aunque se pueda
// redefinir. Con anotaciones de tipo, `@Nullable Object[]` habla de los elementos.
#[test]
fn a_nullable_return_is_reported() {
    assert_eq!(
        findings_in(NULLABLE_RETURNS, "data-flow-issue"),
        ["19:29 clone()", "20:30 length()", "20:58 length()"]
    );
}

const SYSTEM_PROPERTY: &str = r#"package app.base;

import org.jspecify.annotations.Nullable;

public enum Property {
    OS_NAME("os.name");

    private final String key;

    Property(String key) {
        this.key = key;
    }

    public @Nullable String value() {
        return System.getProperty(key);
    }
}
"#;

const REGISTRY: &str = r"package app.base;

import org.jspecify.annotations.Nullable;

public final class Registry {
    public static final Registry DEFAULT = new Registry();

    public @Nullable StringBuilder find() {
        return null;
    }
}
";

const IMPORTED_CONSTANTS: &str = r"package app.client;

import static app.base.Property.OS_NAME;
import static app.base.Registry.DEFAULT;

final class Client {
    int use() {
        return OS_NAME.value().length() + DEFAULT.find().length();
    }

    boolean windows() {
        return OS_NAME.value().startsWith(System.lineSeparator());
    }
}
";

// La raíz de la cadena puede ser una constante importada con `import static`: un valor del enum o
// un campo estático, del tipo que declaran.
#[test]
fn an_imported_constant_is_resolved_as_the_chain_root() {
    assert_eq!(
        findings_in_files(
            &[SYSTEM_PROPERTY, REGISTRY, IMPORTED_CONSTANTS],
            "data-flow-issue"
        ),
        ["8:32 length()", "8:58 length()", "12:32 startsWith()"]
    );
}

const PASSED_CALLS: &str = r"package app.client;

import static app.base.Property.OS_NAME;

import org.jspecify.annotations.Nullable;

final class Splitting {
    static int measure(String text) {
        return text.length();
    }

    static int tolerate(String text) {
        return text == null ? 0 : text.length();
    }

    static char @Nullable [] escape(int code) {
        return null;
    }

    static int size(char @Nullable [] escaped) {
        return escaped.length;
    }

    int use() {
        return measure(OS_NAME.value()) + tolerate(OS_NAME.value()) + measure(String.valueOf(1));
    }

    int escaped() {
        return size(escape(1));
    }
}
";

// Una llamada que puede devolver null pasada directo a un parámetro que el método desreferencia,
// salvo que el parámetro se declare `@Nullable` (en un arreglo, `char @Nullable []`).
#[test]
fn a_nullable_call_passed_to_a_non_null_parameter_is_reported() {
    assert_eq!(
        findings_in_files(&[SYSTEM_PROPERTY, PASSED_CALLS], "null-argument"),
        ["25:24 text"]
    );
}
