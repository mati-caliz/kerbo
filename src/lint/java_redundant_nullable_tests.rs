use super::tests::{expected, lints, rendered_where, strict};
use crate::rules::{JavaRule, RuleId};

fn redundant_nullables(source: &str) -> Vec<String> {
    rendered_where(&lints(source, &strict()), |rule| {
        rule == RuleId::Java(JavaRule::RedundantNullable)
    })
}

// Un método que nadie puede redefinir, anotado `@Nullable` o `@CheckForNull`, que siempre devuelve
// algo no nulo: se marca el nombre de la anotación.
#[test]
fn a_closed_nullable_method_that_never_returns_null_is_reported() {
    let source = "import javax.annotation.CheckForNull;
import javax.annotation.Nullable;

final class Closed {
    public @Nullable String literal() {
        return \"\";
    }
}

class Open {
    @CheckForNull
    private java.io.File local(String name, boolean plain) {
        java.io.File file = new java.io.File(name);
        if (plain) {
            return (file);
        }
        return plain ? this.other() == null ? file : file : (java.io.File) file;
    }

    @javax.annotation.Nullable
    static Object joined(String left) {
        return left + 1;
    }

    final <T> @Nullable Object self() {
        return this;
    }

    Object anonymous = new Object() {
        @Nullable
        public String toString() {
            return \"x\";
        }
    };

    private java.io.File other() {
        return null;
    }
}
";
    assert_eq!(
        redundant_nullables(source),
        expected(
            "11:6 redundant-nullable local
20:6 redundant-nullable joined
25:16 redundant-nullable self
30:10 redundant-nullable toString
5:13 redundant-nullable literal"
        )
    );
}

// No se marca si el método se puede redefinir, si algún `return` puede dar null (un `null`, una
// llamada, un parámetro, una local reasignada o sin valor inicial), si no devuelve nada, si
// devuelve un arreglo o `Void`, si una lambda del cuerpo tiene sus propios retornos o si la
// anotación no es de un paquete que IntelliJ conozca.
#[test]
fn a_nullable_method_that_may_return_null_or_be_overridden_is_not_reported() {
    let source = "import javax.annotation.Nullable;

class Open {
    @Nullable
    public String overridable() {
        return \"\";
    }

    @Nullable
    private String literal(boolean empty) {
        if (empty) {
            return \"\";
        }
        return null;
    }

    @Nullable
    private String called() {
        return String.valueOf(1);
    }

    @Nullable
    private String parameter(String given) {
        return given;
    }

    @Nullable
    private String reassigned(String given) {
        String kept = \"\";
        kept = given;
        return kept;
    }

    @Nullable
    private String late(String given) {
        String kept;
        kept = \"\";
        return kept;
    }

    @Nullable
    private String branch(String given) {
        return given.isEmpty() ? \"\" : given;
    }

    @Nullable
    private static String[] array() {
        return new String[0];
    }

    @Nullable
    private Void nothing() {
        throw new IllegalStateException();
    }

    @Nullable
    private Runnable deferred() {
        Runnable task = () -> { };
        return new Thread(task);
    }

    @Nullable
    private String scoped(boolean inner) {
        if (inner) {
            String field = \"\";
        }
        return field;
    }

    @org.apache.maven.api.annotations.Nullable
    private String unknown() {
        return \"\";
    }

    private String field;
}
";
    assert_eq!(redundant_nullables(source), Vec::<String>::new());
}
