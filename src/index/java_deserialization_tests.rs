use super::java_null_tests::findings_in;

fn sorted(mut findings: Vec<String>) -> Vec<String> {
    findings.sort();
    findings
}

const SERIALIZED_FORMS: &str = r"package app;

import java.io.ObjectInputStream;
import java.io.Serializable;

final class SerializedForm implements Serializable {
    static final Object EMPTY = new Object();
    final Object[] elements;
    final int[] counts;
    final int size;

    SerializedForm(Object[] elements, int[] counts, int size) {
        this.elements = elements;
        this.counts = counts;
        this.size = size;
    }

    Object readResolve() {
        int total = elements.length + counts[0] + size + EMPTY.hashCode();
        return elements.clone();
    }

    Object other() {
        return elements.clone();
    }
}

final class Guarded implements java.io.Serializable {
    final Object value;

    Guarded(Object value) {
        this.value = value;
    }

    private void readObject(ObjectInputStream stream) {}

    Object readResolve() {
        return value.toString();
    }
}

final class Plain {
    final Object value;

    Plain(Object value) {
        this.value = value;
    }

    Object readResolve() {
        return value.toString();
    }
}
";

// En el `readResolve()` de una clase `Serializable` sin `readObject` los campos de instancia que no
// son primitivos pueden ser null, cada uno marcado en su primer uso; los estáticos y los demás
// métodos no cambian.
#[test]
fn fields_read_in_read_resolve_may_be_null() {
    assert_eq!(
        sorted(findings_in(SERIALIZED_FORMS, "data-flow-issue")),
        ["19:21 elements()", "19:39 counts()"]
    );
}

const GUARDED_LOOP: &str = r"package app;

import java.io.Serializable;

final class Table implements Serializable {
    final Object[] keys;
    final Object[] values;

    Table(Object[] keys, Object[] values) {
        this.keys = keys;
        this.values = values;
    }

    Object readResolve() {
        if (values.length == 0) {
            return null;
        }
        for (int i = 0; i < values.length; i++) {
            keys[i].hashCode();
        }
        return keys.clone();
    }
}
";

// Un `for` desde cero hasta el largo de un arreglo que un `if` anterior ya descartó vacío da al
// menos una vuelta: lo que el cuerpo desreferencia queda no nulo después.
#[test]
fn a_loop_over_a_non_empty_array_runs_at_least_once() {
    assert_eq!(
        sorted(findings_in(GUARDED_LOOP, "data-flow-issue")),
        ["15:13 values()", "19:13 keys()"]
    );
}
