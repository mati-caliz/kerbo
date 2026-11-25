use super::java_null_tests::findings_in_files;

const SOURCE: &str = r#"package app;

import java.util.Map.Entry;
import org.jspecify.annotations.NullMarked;
import org.jspecify.annotations.Nullable;

@NullMarked
final class Parameters {
    static void accept(Object value) {
        value.hashCode();
    }

    static void unchecked(@Nullable Object value, @Nullable String text) {
        accept(value);
        text.length();
    }

    static void checked(@Nullable Object value, @Nullable String text) {
        if (value != null) {
            accept(value);
        }
        if (text == null) {
            return;
        }
        text.length();
    }

    static int casted(@Nullable Object first, @Nullable Object second) {
        accept((String) first);
        return ((Comparable<Object>) second).compareTo(first);
    }

    static void plain(Object value, String @Nullable [] texts) {
        accept(value);
        accept(texts);
    }

    static void elements(@Nullable Object... data) {
        for (int i = 0; i < data.length; i++) {
            accept((String) data[i]);
            accept(data[0]);
        }
    }

    static void entries(int n, @Nullable Entry<String, String>[] entries) {
        for (int i = 0; i < n; i++) {
            entries[i] = Map.entry("a", "b");
            String key = entries[i].getKey();
            accept(entries[i]);
        }
    }

    static void guarded(@Nullable String value, @Nullable Object other) {
        if (isNullOrEmpty(value)) {
            throw new IllegalArgumentException();
        }
        value.length();
        if (Parameters.class.isInstance(other)) {
            other.hashCode();
        }
    }

    static boolean isNullOrEmpty(@Nullable String text) {
        return text == null || text.isEmpty();
    }
}

final class Unmarked {
    static String format(String format, Object... args) {
        return args.toString() + format;
    }

    static String message(@Nullable String value) {
        return format("%s", value);
    }
}
"#;

// Un parámetro `@Nullable` llega posiblemente null: pasarlo sin chequear a un parámetro no nulo o
// desreferenciarlo se marca, también a través de un casteo, donde se marca la variable. Los
// elementos de un arreglo `@Nullable` leídos con un índice variable siempre pueden ser null; con un
// índice constante IntelliJ los sigue y no se marcan. `isNullOrEmpty` e `isInstance` lo chequean, y
// un parámetro que no es arreglo va a un varargs como elemento.
#[test]
fn a_nullable_parameter_is_possibly_null() {
    let mut arguments = findings_in_files(&[SOURCE], "null-argument");
    arguments.sort();
    assert_eq!(
        arguments,
        [
            "14:16 value",
            "29:25 value",
            "35:16 value",
            "40:29 value",
            "49:20 value"
        ]
    );
    let mut dereferences = findings_in_files(&[SOURCE], "data-flow-issue");
    dereferences.sort();
    assert_eq!(
        dereferences,
        ["15:14 length()", "30:46 compareTo()", "48:37 getKey()"]
    );
}

const CASTS: &str = r"package app;

import org.jspecify.annotations.NullMarked;
import org.jspecify.annotations.Nullable;

@NullMarked
final class NullnessCasts {
    static <T extends @Nullable Object> T uncheckedCastNullableTToT(@Nullable T t) {
        return t;
    }
}
";

const FORWARDING: &str = r"package app;

import static app.NullnessCasts.uncheckedCastNullableTToT;

import org.jspecify.annotations.NullMarked;
import org.jspecify.annotations.Nullable;

@NullMarked
abstract class Converter<A> {
    abstract String doForward(A a);

    String unsafe(@Nullable A a) {
        return doForward(uncheckedCastNullableTToT(a));
    }

    String present(A a) {
        return doForward(uncheckedCastNullableTToT(a));
    }

    abstract @Nullable A lookup(String key);

    String looked(String key) {
        return doForward(uncheckedCastNullableTToT(lookup(key)));
    }

    int assigned(@Nullable A a) {
        A copy = uncheckedCastNullableTToT(a);
        return copy.hashCode();
    }
}
";

// Un método que sólo devuelve su parámetro (`uncheckedCastNullableTToT`) es tan nulo como su
// argumento: un parámetro `@Nullable` o una llamada que puede devolver null, pasados así a un
// parámetro no nulo o guardados y desreferenciados. Con un argumento no nulo, no.
#[test]
fn a_method_returning_its_parameter_forwards_a_possibly_null_argument() {
    let mut arguments = findings_in_files(&[CASTS, FORWARDING], "null-argument");
    arguments.sort();
    assert_eq!(arguments, ["13:26 a", "23:26 a"]);
    let dereferences = findings_in_files(&[CASTS, FORWARDING], "data-flow-issue");
    assert_eq!(dereferences, ["28:21 hashCode()"]);
}

const ANNOTATED_PARAMETERS: &str = r"package app;

import edu.umd.cs.findbugs.annotations.CheckForNull;
import edu.umd.cs.findbugs.annotations.NonNull;

public class Symlinks {
    public void create(@NonNull String target, String listener) {
        listener.hashCode();
        target.hashCode();
    }

    public @CheckForNull String find(String name) {
        return name;
    }

    void use(Symlinks other) {
        other.create(null, null);
        other.create(find(other.toString()), other.toString());
    }
}
";

// Un parámetro que el proyecto anota `@NonNull` no admite un `null` ni una llamada que puede darlo,
// aunque el método se pueda redefinir; el que no se anota sólo cuenta si nadie lo redefine.
#[test]
fn a_project_parameter_annotated_non_null_rejects_null() {
    let mut arguments = findings_in_files(&[ANNOTATED_PARAMETERS], "null-argument");
    arguments.sort();
    assert_eq!(arguments, ["17:22 target", "18:22 target"]);
}

const OVERRIDDEN: &str = r"package app;

import org.jspecify.annotations.Nullable;

abstract class Formatter {
    abstract void translate(@Nullable String markup);
}

final class Escaped extends Formatter {
    @Override
    void translate(String markup) {
        markup.length();
    }
}

final class Plain extends Formatter {
    void translate(String markup) {
        markup.length();
    }
}

final class Declared extends Formatter {
    @Override
    void translate(@NonNull String markup) {
        markup.length();
    }
}
";

// Un parámetro sin anotación hereda el `@Nullable` del método que redefine, con `@Override` o sin él
// no importa para IntelliJ; acá sólo se sigue con `@Override`. Uno anotado `@NonNull` no lo hereda.
#[test]
fn a_parameter_inherits_nullable_from_the_overridden_method() {
    assert_eq!(
        findings_in_files(&[OVERRIDDEN], "data-flow-issue"),
        ["12:16 length()"]
    );
}
