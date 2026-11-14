use super::java_null_tests::{findings_in, findings_in_files};

const LISTS: &str = r"package app;

import java.util.List;
import org.jspecify.annotations.NullMarked;
import org.jspecify.annotations.Nullable;

@NullMarked
abstract class Lists<E> {
    abstract List<E> getList();

    void call(List<String> names, List<@Nullable String> nullable, List<?> unknown) {
        names.set(0, null);
        nullable.set(0, null);
        unknown.set(0, null);
        getList().set(0, null);
        names.add(null);
    }
}

final class Unmarked {
    void call(List<String> names) {
        names.set(0, null);
    }
}
";

// Bajo `@NullMarked`, un parámetro de un método del JDK cuyo tipo es una variable de la clase no
// admite null si el receptor (una variable o un getter del archivo) se declara con un tipo concreto
// o una variable sin límite `@Nullable` en su lugar; no con `@Nullable`, un comodín, ni sin marca.
#[test]
fn a_null_passed_to_a_substituted_library_variable_is_reported() {
    let mut found = findings_in(LISTS, "null-argument");
    found.sort();
    assert_eq!(found, ["12:22 E", "15:26 E", "16:19 E"]);
}

const LOOPS: &str = r"package app;

import java.util.List;
import java.util.Optional;
import org.jspecify.annotations.NullMarked;
import org.jspecify.annotations.Nullable;

@NullMarked
final class Loops {
    @Nullable String find() {
        return null;
    }

    void call(List<String> names, Optional<String> maybe) {
        int size = 3;
        String next = find();
        for (int i = 0; i <= size; i++) {
            names.add(next);
        }
        names.add(next);
        maybe.orElse(null);
        int changing = 3;
        changing++;
        String other = find();
        for (int i = 0; i < changing; i++) {
            names.add(other);
        }
        names.add(other);
    }
}
";

// Un `for` desde cero hasta un número, o hasta una local inicializada con uno que no se reasigna,
// da al menos una vuelta: lo que el cuerpo pasó a un parámetro no nulo queda no nulo después.
// `Optional.orElse` admite null.
#[test]
fn a_loop_up_to_a_constant_runs_at_least_once() {
    let mut found = findings_in(LOOPS, "null-argument");
    found.sort();
    assert_eq!(found, ["18:23 E", "26:23 E", "28:19 E"]);
}

const BASE_TESTER: &str = r"package app;

import java.util.Collection;
import java.util.Map;

abstract class BaseTester<K, V> {
    protected Collection<K> collection;

    protected Map<K, V> getMap() {
        return null;
    }
}
";

const CHILD_TESTER: &str = r"package app;

final class ChildTester<K, V> extends BaseTester<K, V> {
    void call(K key, V value) {
        collection.removeAll(null);
        getMap().putAll(null);
        getMap().merge(key, value, null);
        getMap().merge(key, null, (left, right) -> left);
        getMap().put(key, null);
    }
}
";

// Un campo o un getter heredados de una clase del proyecto llevan al tipo que declaran: los
// parámetros del JDK que IntelliJ da por no nulos se marcan también ahí.
#[test]
fn an_inherited_receiver_reaches_the_library_method() {
    let mut found = findings_in_files(&[BASE_TESTER, CHILD_TESTER], "null-argument");
    found.sort();
    assert_eq!(
        found,
        ["5:30 c", "6:25 m", "7:36 remappingFunction", "8:29 value"]
    );
}

const GETTER: &str = r#"package app;

import org.jspecify.annotations.Nullable;

abstract class Base<H> {
    private @Nullable H subscriber;

    H getSubscriber() {
        return subscriber;
    }
}

final class Holder {
    final String name = "";
}

final class Child extends Base<Holder> {
    void call() {
        Object first = getSubscriber().name;
        Object second = getSubscriber().name;
        getSubscriber().toString();
    }
}

final class Other extends Base<String> {
    void call() {
        getSubscriber().length();
        getSubscriber().length();
    }
}
"#;

// Un getter cuyo cuerpo sólo devuelve un campo `@Nullable` se lee como el campo, aunque se pueda
// redefinir: la primera desreferencia (una llamada o la lectura de un campo, marcada en el
// calificador) se marca y después ya se lo da por presente.
#[test]
fn a_simple_getter_of_a_nullable_field_may_return_null() {
    let mut found = findings_in(GETTER, "data-flow-issue");
    found.sort();
    assert_eq!(found, ["19:24 getSubscriber()", "27:25 length()"]);
}

const INFERRED: &str = r"package app;

import java.util.Map;
import java.util.function.Function;
import org.jspecify.annotations.NullMarked;
import org.jspecify.annotations.Nullable;

@NullMarked
final class Entries {
    static <K extends @Nullable Object, V extends @Nullable Object> Map.Entry<K, V> entry(K key, V value) {
        throw new UnsupportedOperationException();
    }

    static <E extends @Nullable Object> Function<@Nullable Object, E> constant(E value) {
        throw new UnsupportedOperationException();
    }

    static <K, V> void index(Iterable<K> keys, Function<? super K, V> function) {}
}

@NullMarked
final class Builder<K, V> {
    void put(Map.Entry<? extends K, ? extends V> entry) {}

    void putLoose(Map.Entry<? extends @Nullable K, ? extends V> entry) {}
}

@NullMarked
final class Caller {
    void call(Builder<String, Integer> builder, Builder<@Nullable String, Integer> loose, Iterable<String> keys) {
        builder.put(Entries.entry(null, 1));
        builder.put(Entries.entry(1 > 0 ? null : null, null));
        builder.putLoose(Entries.entry(null, 1));
        loose.put(Entries.entry(null, 1));
        Entries.index(keys, Entries.constant(null));
    }
}
";

// Un `null` pasado a una variable de tipo del método de adentro se marca si la llamada es
// argumento de otra cuyo parámetro pone algo no nulo en ese lugar: un argumento de tipo del
// receptor no nulo o una variable del método sin límite `@Nullable`.
#[test]
fn a_null_inferred_non_null_from_the_outer_call_is_reported() {
    let mut found = findings_in(INFERRED, "null-argument");
    found.sort();
    assert_eq!(found, ["31:35 K", "32:56 V", "35:46 E"]);
}

const SHADOWED: &str = r"package app;

import java.util.List;
import java.util.Set;

final class Holder {
    private List<String> items;

    void elsewhere(Set<Integer> items) {
        items.size();
    }

    void call() {
        items.addAll(null);
    }
}
";

// El receptor que el método no declara es el campo de la clase, aunque otro método tenga un
// parámetro del mismo nombre con otro tipo.
#[test]
fn a_field_is_the_receiver_when_the_method_does_not_declare_it() {
    assert_eq!(findings_in(SHADOWED, "null-argument"), ["14:22 c"]);
}

const PARAMETRIC: &str = r"package app;

import org.jspecify.annotations.NullMarked;
import org.jspecify.annotations.Nullable;

@NullMarked
interface Instances<B extends @Nullable Object> {
    <T extends B> void putInstance(Class<T> type, @ParametricNullness T value);
}

@NullMarked
final class Caller {
    void call(Instances<Number> map) {
        map.putInstance(Integer.class, null);
        map.putInstance(null, 1);
    }
}
";

// Un parámetro `@ParametricNullness` admite null si su variable lo admite: no se marca.
#[test]
fn a_parametric_nullness_parameter_admits_null() {
    assert_eq!(findings_in(PARAMETRIC, "null-argument"), ["15:25 type"]);
}

const NULLABLE_ARGUMENTS: &str = r"package app;

import java.util.List;
import java.util.Queue;
import org.jspecify.annotations.NullMarked;
import org.jspecify.annotations.Nullable;

@NullMarked
interface Box<T extends @Nullable Object> {
    T get();
}

@NullMarked
final class Reader {
    void read(Box<@Nullable String> loose, Box<String> strict, List<@Nullable String> names, List<String> plain, Queue<String> queue) {
        loose.get().length();
        strict.get().length();
        names.get(0).length();
        plain.get(0).length();
        queue.peek().hashCode();
    }
}
";

// Un método que devuelve una variable de la clase puede devolver null si el receptor la declaró
// `@Nullable` en ese lugar, en el proyecto y en el JDK; y `Queue.peek` puede devolver null.
#[test]
fn a_nullable_type_argument_of_the_receiver_reaches_what_the_method_returns() {
    let mut found = findings_in(NULLABLE_ARGUMENTS, "data-flow-issue");
    found.sort();
    assert_eq!(found, ["16:21 length()", "18:22 length()", "20:22 hashCode()"]);
}

const CALLED_RECEIVER: &str = r#"package app;

import java.io.File;
import java.util.List;
import org.jspecify.annotations.NullMarked;
import org.jspecify.annotations.Nullable;

@NullMarked
final class Splitter {
    static Splitter on(String separator) {
        return new Splitter();
    }

    Iterable<String> split(CharSequence text) {
        return List.of();
    }
}

@NullMarked
final class Reader {
    @Nullable String value() {
        return null;
    }

    @Nullable String path() {
        return null;
    }

    void read() {
        Splitter.on(",").split(value());
        Splitter.on(",").split(null);
        new File(path());
    }
}
"#;

// Un receptor que es otra llamada sobre un tipo conocido se resuelve por lo que devuelve, y
// `new File(...)` no admite null (anotación externa).
#[test]
fn a_called_receiver_is_resolved_by_what_it_returns() {
    let mut found = findings_in(CALLED_RECEIVER, "null-argument");
    found.sort();
    assert_eq!(found, ["30:32 text", "31:32 text", "32:18 pathname"]);
}

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
