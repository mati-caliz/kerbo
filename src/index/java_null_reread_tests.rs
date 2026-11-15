use super::java_null_tests::findings_in;

const FORGOTTEN: &str = r"package app;

import org.jspecify.annotations.Nullable;

final class Segment {
    volatile @Nullable StringBuilder table;
    @Nullable StringBuilder fixed;
    void expand() {}
}

final class Forgetful {
    volatile @Nullable StringBuilder own;

    void calls(Segment segment) {
        segment.table.length();
        segment.table.length();
        segment.expand();
        segment.table.length();
        segment.fixed.length();
        segment.expand();
        segment.fixed.length();
        if (own != null) {
            own.length();
            segment.expand();
            own.length();
        }
    }
}
";

// IntelliJ no sigue un campo `@Nullable` y `volatile`: cada lectura puede ser null aunque ya se
// haya chequeado o desreferenciado. Uno que no es `volatile` se sigue como una variable.
#[test]
fn a_volatile_nullable_field_is_possibly_null_on_every_read() {
    let mut found = findings_in(FORGOTTEN, "data-flow-issue");
    found.sort();
    assert_eq!(
        found,
        [
            "15:23 length()",
            "16:23 length()",
            "18:23 length()",
            "19:23 length()",
            "23:17 length()",
            "25:17 length()"
        ]
    );
}

const REASSIGNED: &str = r"package app;

import java.util.concurrent.CountDownLatch;
import org.jspecify.annotations.NullMarked;
import org.jspecify.annotations.Nullable;

@NullMarked
final class Closeables {
    private volatile @Nullable CountDownLatch whenClosed;
    private volatile @Nullable Exception failure;
    private @Nullable Exception plain;

    CountDownLatch whenClosedCountDown() {
        whenClosed = new CountDownLatch(1);
        return whenClosed;
    }

    static @Nullable Exception rethrowAsException(Exception cause) throws Exception {
        throw cause;
    }

    void rethrowCall(Exception cause) throws Exception {
        throw rethrowAsException(cause);
    }

    void rethrow() throws Exception {
        if (failure != null) {
            throw failure;
        }
        if (plain != null) {
            throw plain;
        }
        throw plain;
    }
}
";

// Asignar un campo `volatile` no lo deja presente: lo que se devuelve es otra lectura. `throw x`, y
// `throw` de una llamada que puede devolver null, desreferencian lo que tiran.
#[test]
fn a_reassigned_volatile_field_is_still_possibly_null() {
    assert_eq!(findings_in(REASSIGNED, "nullable-return"), ["15:16 whenClosed"]);
    let mut thrown = findings_in(REASSIGNED, "data-flow-issue");
    thrown.sort();
    assert_eq!(
        thrown,
        ["23:15 rethrowAsException()", "28:19 failure()", "33:15 plain()"]
    );
}

const THROUGH_ARRAYS: &str = r"package app;

import org.jspecify.annotations.Nullable;

final class Holder {
    volatile @Nullable StringBuilder table;
    @Nullable StringBuilder plain;
}

final class Store {
    final Holder[] holders = new Holder[2];
}

final class ArrayUses {
    int use(Store store) {
        return store.holders[0].table.length() + store.holders[1].plain.length();
    }
}
";

// Un campo `@Nullable` al que se llega por un elemento de un arreglo que es campo de una variable.
#[test]
fn a_nullable_field_through_an_array_element_is_reported() {
    assert_eq!(
        findings_in(THROUGH_ARRAYS, "data-flow-issue"),
        ["16:39 length()", "16:73 length()"]
    );
}

const CASTS: &str = r"package app;

import org.jspecify.annotations.Nullable;

final class Heap {
    @Nullable Integer peek() {
        return null;
    }

    <T extends Number> @Nullable T typed(Class<T> type) {
        return type.cast(peek());
    }

    static int casts(Heap heap) {
        return (int) heap.peek() + (int) (Object) heap.peek() + (int) heap.typed(Integer.class);
    }

    static int variable(Heap heap) {
        Integer value = heap.peek();
        return (int) value;
    }
}
";

// Castear a un primitivo lo que puede devolver null lo desempaqueta; castearlo a `Object` no. El
// `@Nullable` de un método genérico va después de sus parámetros de tipo.
#[test]
fn a_cast_to_a_primitive_unboxes_a_nullable_result() {
    assert_eq!(
        findings_in(CASTS, "data-flow-issue"),
        ["15:22 peek()", "15:71 typed()", "20:22 value()"]
    );
}

const UNBOXED_ARGUMENTS: &str = r"package app;

import org.jspecify.annotations.Nullable;

final class Queue {
    @Nullable Integer poll() {
        return null;
    }

    static void remove(int value) {}

    static void store(Integer value) {}

    static int uses(Queue queue) {
        remove(queue.poll());
        store(queue.poll());
        return (queue.poll() ^ 1) + (queue.poll() + 1);
    }
}
";

// Pasar a un parámetro primitivo o usar en una cuenta lo que puede devolver null lo desempaqueta;
// pasarlo a un `Integer` o sumarlo (que puede ser concatenar) no.
#[test]
fn a_nullable_result_passed_to_a_primitive_or_operated_is_unboxed() {
    assert_eq!(
        findings_in(UNBOXED_ARGUMENTS, "data-flow-issue"),
        ["15:16 poll()", "17:17 poll()"]
    );
}
