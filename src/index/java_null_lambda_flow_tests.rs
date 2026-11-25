use super::java_null_tests::findings_in;

const SESSIONS: &str = r"package app;

import org.jspecify.annotations.Nullable;

final class Sessions {
    interface Step {
        void run(String name);
    }

    static @Nullable String find() {
        return null;
    }

    void then(Step step) {
    }

    void call(String outer) {
        String captured = find();
        then(name -> {
            String found = find();
            found.length();
            String checked = find();
            if (checked != null) {
                checked.length();
            }
            then(inner -> {
                String nested = find();
                nested.trim();
            });
            outer.length();
        });
        then(name -> name.length());
        captured.length();
    }
}
";

// El cuerpo de una lambda se recorre como un método aparte: una variable que declara ella y sale
// de una llamada que puede dar null se marca al usarla sin chequear, también en una lambda
// anidada. Lo que viene de afuera (`outer`) no se sigue ahí adentro.
#[test]
fn a_nullable_variable_declared_in_a_lambda_is_followed_inside_it() {
    assert_eq!(
        findings_in(SESSIONS, "data-flow-issue"),
        ["21:19 length()", "28:24 trim()", "33:18 length()"]
    );
}

const LOOPS: &str = r"package app;

import java.util.List;
import org.jspecify.annotations.Nullable;

final class Loops {
    private @Nullable String label;

    static @Nullable String find() {
        return null;
    }

    void then(Runnable step) {
    }

    void call(List<String> names) {
        String waited;
        while ((waited = find()) == null) {
            names.clear();
        }
        waited.length();
        for (int i = 0; i < 3; i++) {
            String item = find();
            if (item == null) {
                names.clear();
            }
        }
        for (String item : names) {
            item.length();
        }
        String kept = null;
        while (names.isEmpty()) {
            kept = find();
        }
        kept.length();
        String label = names.toString();
        then(() -> {
            label.length();
            this.label.length();
        });
    }
}
";

// Un `while` se deja cuando su condición no se cumple: la variable que asigna y compara sale
// presente, y la que asigna el cuerpo puede salir null. La variable de un `for (x : algo)` no
// hereda lo de otra con el mismo nombre. Dentro de una lambda, una local del método tapa al campo
// `@Nullable` del mismo nombre, que sigue siendo posiblemente null como `this.label`.
#[test]
fn loops_leave_through_their_condition_and_locals_shadow_fields_inside_lambdas() {
    assert_eq!(
        findings_in(LOOPS, "data-flow-issue"),
        ["35:14 length()", "39:24 length()"]
    );
}

const STATIC_BLOCK: &str = r"package app;

import org.jspecify.annotations.Nullable;

final class Holder {
    static final int SIZE;

    static @Nullable String maybe() {
        return null;
    }

    static {
        String name = maybe();
        SIZE = name.length();
    }
}
";

// El bloque `static` se recorre como un método: una variable asignada desde una llamada nula y
// usada sin chequear se marca igual.
#[test]
fn a_static_initializer_block_is_analyzed() {
    assert_eq!(findings_in(STATIC_BLOCK, "data-flow-issue"), ["14:21 length()"]);
}
