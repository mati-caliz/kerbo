use super::java_null_tests::findings_in_files;

const TABLES: &str = r"package app;

import java.util.function.BinaryOperator;
import java.util.function.Function;
import org.jspecify.annotations.NullMarked;
import org.jspecify.annotations.Nullable;

@NullMarked
final class Tables {
    static <T extends @Nullable Object, R, V extends @Nullable Object> Object collect(
            Function<? super T, ? extends R> row,
            Function<? super T, ? extends V> value,
            BinaryOperator<R> merge) {
        return row;
    }

    static <V> Object submit(AsyncCallable<V> callable) {
        return callable;
    }

    static <R> Object nullable(Function<String, ? extends @Nullable R> function) {
        return function;
    }
}

@NullMarked
interface AsyncCallable<V extends @Nullable Object> {
    Holder<V> call() throws Exception;
}

@NullMarked
interface Holder<V extends @Nullable Object> {}
";

const CALLER: &str = r#"package app;

import java.security.PrivilegedExceptionAction;
import java.util.function.Supplier;
import org.jspecify.annotations.NullMarked;
import org.jspecify.annotations.NullUnmarked;
import org.jspecify.annotations.Nullable;

@NullMarked
final class Caller {
    void typed() {
        Object row = Tables.collect(t -> null, t -> null, (a, b) -> null);
        Tables.collect(t -> null, t -> "", (a, b) -> "");
        Tables.submit(() -> null);
        Object nullable = Tables.nullable(s -> null);
        PrivilegedExceptionAction<String> action = () -> {
            if (row == null) {
                return null;
            }
            Supplier<@Nullable String> inner = () -> null;
            return "";
        };
    }
}

@NullUnmarked
final class Unmarked {
    void call() {
        Object row = Tables.collect(t -> null, t -> null, (a, b) -> null);
        Supplier<String> supplier = () -> null;
        Tables.submit(() -> null);
    }
}
"#;

// Una lambda que devuelve `null` se marca si su interfaz funcional, ya sustituida, devuelve algo
// no nulo bajo `@NullMarked`: un tipo concreto de una interfaz del proyecto, un tipo concreto
// escrito en una variable declarada, o una variable de tipo del método sin límite `@Nullable`
// cuando la llamada tiene un tipo destino. La lambda escrita en código sin marcar no se marca.
#[test]
fn a_lambda_returning_null_to_a_non_null_functional_return_is_reported() {
    let mut found = findings_in_files(&[TABLES, CALLER], "nullable-return");
    found.sort();
    assert_eq!(found, ["12:42 null", "12:69 null", "14:29 null", "18:24 null"]);
}
