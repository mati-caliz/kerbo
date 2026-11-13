use super::java_null_tests::{findings_in, findings_in_files};

const RETURNS: &str = r#"package app;

import org.jspecify.annotations.NullMarked;
import org.jspecify.annotations.Nullable;

@NullMarked
final class Names {
    private @Nullable String cached;

    @Nullable String find(String key) {
        return key.isEmpty() ? null : key;
    }

    String literal() {
        return null;
    }

    String field() {
        return cached;
    }

    String call(String key) {
        return find(key);
    }

    String checked() {
        if (cached == null) {
            return "";
        }
        return cached;
    }

    @Nullable String allowed() {
        return null;
    }

    int primitive() {
        return 0;
    }

    <T extends @Nullable Object> T parametric(@Nullable T value) {
        return value;
    }

    String lambda() {
        java.util.function.Supplier<@Nullable String> supplier = () -> null;
        return "x";
    }
}

final class Unmarked {
    String literal() {
        return null;
    }
}
"#;

// Bajo `@NullMarked`, un método sin `@Nullable` no devuelve null: se marcan el `null` escrito, el
// campo `@Nullable` sin chequear y la llamada que puede devolverlo; no los métodos que admiten null,
// los primitivos, las variables de tipo con límite `@Nullable`, las lambdas ni lo que no se marca.
#[test]
fn a_null_returned_by_a_null_marked_method_is_reported() {
    let mut found = findings_in(RETURNS, "nullable-return");
    found.sort();
    assert_eq!(found, ["15:16 null", "19:16 cached", "23:16 find(key)"]);
}

const PACKAGE_INFO: &str = r"@NullMarked
package app.base;

import org.jspecify.annotations.NullMarked;
";

const CONVERTER: &str = r"package app.base;

import org.jspecify.annotations.Nullable;

public abstract class Converter<A, B> {
    private volatile @Nullable B cached;

    public final @Nullable B convert(@Nullable A a) {
        return a == null ? null : doForward(a);
    }

    protected abstract B doForward(A a);

    public final B apply(A a) {
        return convert(a);
    }

    B last() {
        return cached;
    }

    @Nullable B nothing() {
        return null;
    }

    Object literal() {
        return null;
    }
}
";

#[test]
fn the_package_marks_what_its_methods_return() {
    let mut found = findings_in_files(&[PACKAGE_INFO, CONVERTER], "nullable-return");
    found.sort();
    assert_eq!(found, ["15:16 convert(a)", "19:16 cached", "27:16 null"]);
}
