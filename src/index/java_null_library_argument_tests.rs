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
