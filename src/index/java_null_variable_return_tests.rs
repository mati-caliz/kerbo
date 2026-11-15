use super::java_null_tests::findings_in_files;

const TESTERS: &str = r"package app;

import java.util.Collection;
import org.jspecify.annotations.NullMarked;
import org.jspecify.annotations.Nullable;

@NullMarked
interface ContainerGenerator<T, E extends @Nullable Object> {
    T create(Object... elements);
}

@NullMarked
interface OneSizeGenerator<T, E extends @Nullable Object> extends ContainerGenerator<T, E> {}

@NullMarked
class AbstractTester<G> {
    G generator;

    public G getSubjectGenerator() {
        return generator;
    }
}

@NullMarked
abstract class AbstractContainerTester<C, E extends @Nullable Object>
        extends AbstractTester<OneSizeGenerator<C, E>> {
    E sample;

    protected final E e0() {
        return sample;
    }

    protected void assertGet(Object key, E... values) {
        key.hashCode();
        values.clone();
    }

    protected void assertGet(Object key, Collection<? extends E> values) {
        key.hashCode();
        values.isEmpty();
    }
}

@NullMarked
abstract class AbstractCollectionTester<E extends @Nullable Object>
        extends AbstractContainerTester<Collection<E>, E> {}

@NullMarked
abstract class Runner<E extends @Nullable Object> {
    protected abstract Object target();
}

@NullMarked
final class Sets {
    static <E extends @Nullable Object> int newHashSet(E... elements) {
        return elements.length;
    }

    static void accept(Object value) {
        value.hashCode();
    }
}

@NullMarked
class Holder<K extends @Nullable Object> {
    K key;

    K getKey() {
        return key;
    }
}
";

const CALLER: &str = r"package app;

import static app.Sets.newHashSet;
import org.jspecify.annotations.NullMarked;
import org.jspecify.annotations.Nullable;

@NullMarked
class IteratorTesterCase<E extends @Nullable Object> extends AbstractCollectionTester<E> {
    void anonymous() {
        new Runner<E>() {
            @Override
            protected Object target() {
                return getSubjectGenerator().create(e0(), e0());
            }
        };
    }

    int single() {
        return Sets.newHashSet(e0());
    }

    int several() {
        return Sets.newHashSet(e0(), e0()) + newHashSet(e0(), e0());
    }

    void inheritedVarargs() {
        assertGet(this, e0());
        assertGet(this, e0(), e0());
    }

    void concrete() {
        new Holder<String>() {
            void check() {
                Sets.accept(getKey());
            }
        };
    }
}

@NullMarked
class NonNullTesterCase<E> extends AbstractCollectionTester<E> {
    void anonymous() {
        new Runner<E>() {
            @Override
            protected Object target() {
                return getSubjectGenerator().create(e0(), e0());
            }
        };
    }
}

class UnmarkedTesterCase<E extends @Nullable Object> extends AbstractCollectionTester<E> {
    Object direct() {
        return getSubjectGenerator().create(e0(), e0());
    }
}
";

// Un método heredado que devuelve la variable de su clase, que desde la clase que llama es una
// variable con límite `@Nullable`, puede devolver null bajo `@NullMarked`; el receptor heredado
// `getSubjectGenerator()` lleva, por los padres genéricos, al `create(Object...)` no nulo. Con una
// variable sin límite `@Nullable`, o sin `@NullMarked` del que llama, no se marca.
#[test]
fn an_inherited_call_returning_a_nullable_type_variable_is_a_possibly_null_argument() {
    let mut found = findings_in_files(&[TESTERS, CALLER], "null-argument");
    found.sort();
    assert_eq!(found, ["13:53 elements", "13:59 elements"]);
}
