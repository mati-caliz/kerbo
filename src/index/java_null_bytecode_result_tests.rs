use super::java_null_bytecode_argument_tests::findings_with_the_jar;

const FORWARDED: &str = r"package app;

import org.jspecify.annotations.Nullable;
import sample.inferred.Results;

final class Forwarded {
    static @Nullable Object maybe() {
        return null;
    }

    static int use(String text) {
        return text.length();
    }

    void variables(Object given) {
        String first = Results.orNull(maybe());
        first.length();
        String second = Results.ofInstance(maybe());
        second.length();
        String third = Results.text(maybe());
        third.length();
        String fourth = Results.orEmpty(maybe());
        fourth.length();
        String fifth = Results.guarded(maybe());
        fifth.length();
        String sixth = Results.orNull(given);
        sixth.length();
    }

    void arguments(Object given) {
        use(Results.orNull(maybe()));
        use(Results.text(maybe()));
        use(Results.orNull(given));
    }
}
";

// Un método estático de un jar que con null devuelve siempre null (`if (value == null) return
// null;`, o un `instanceof` que no pasa) da null si lo que recibe puede serlo. No si con null
// devuelve otra cosa, si tiene un `try` o si el argumento no se sabe posiblemente null.
#[test]
fn a_library_method_that_returns_null_for_null_forwards_a_nullable_argument() {
    assert_eq!(
        findings_with_the_jar(FORWARDED, "data-flow-issue"),
        ["17:15 length()", "19:16 length()"]
    );
    assert_eq!(findings_with_the_jar(FORWARDED, "null-argument"), ["31:13 text"]);
}
