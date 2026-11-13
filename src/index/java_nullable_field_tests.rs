use super::java_null_tests::findings_in;

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
