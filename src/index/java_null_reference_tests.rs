use super::java_null_tests::findings_in;

const REFERENCES: &str = r#"package app;

import java.util.function.BiFunction;
import org.jspecify.annotations.NullMarked;
import org.jspecify.annotations.Nullable;

@NullMarked
final class Ops {
    static int sum(int left, int right) { return left + right; }
    static String join(String left, String right) { return left + right; }
}

@NullMarked
interface Ranges<V extends @Nullable Object> {
    void merge(@Nullable V value, BiFunction<? super V, ? super @Nullable V, ? extends @Nullable V> function);
}

@NullMarked
final class Caller {
    void call(Ranges<Integer> ranges, Ranges<String> texts) {
        ranges.merge(1, Ops::sum);
        texts.merge("", Ops::join);
    }
}
"#;

// Una referencia a un método con parámetros primitivos pasada donde la interfaz funcional le da
// algo `@Nullable` se desempaqueta: se marca la referencia.
#[test]
fn a_method_reference_that_unboxes_a_nullable_argument_is_reported() {
    assert_eq!(findings_in(REFERENCES, "data-flow-issue"), ["21:25 sum()"]);
}

const AMBIGUOUS_ROOT: &str = r#"package app;

import java.util.List;
import org.jspecify.annotations.NullMarked;
import org.jspecify.annotations.Nullable;

@NullMarked
interface Instances {
    @Nullable Integer find(String key);
}

@NullMarked
final class Holder {
    private Instances map;

    void other(List<String> map) {
        map.size();
    }

    void call() {
        int value = (int) map.find("a");
    }
}
"#;

// La raíz de una cadena declarada con tipos distintos en el archivo toma el tipo del alcance del
// uso: acá, el campo.
#[test]
fn a_chain_root_takes_the_type_in_scope() {
    assert_eq!(findings_in(AMBIGUOUS_ROOT, "data-flow-issue"), ["21:27 find()"]);
}

const GUARDED_QUEUE: &str = r"package app;

import java.util.Queue;

final class Runner {
    void run(Queue<Runnable> tasks) {
        while (!tasks.isEmpty()) {
            tasks.poll().run();
        }
    }

    void unguarded(Queue<Runnable> tasks) {
        tasks.poll().run();
    }
}
";

// Un `poll()` dentro de un `while (!cola.isEmpty())` no avisa: IntelliJ sigue el tamaño.
#[test]
fn a_poll_guarded_by_a_non_empty_check_is_not_reported() {
    assert_eq!(findings_in(GUARDED_QUEUE, "data-flow-issue"), ["13:22 run()"]);
}
