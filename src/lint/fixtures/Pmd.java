package fixtures;

import java.io.IOException;
import java.io.StringReader;
import java.lang.String;
import java.util.Collection;
import java.util.Deque;
import java.util.List;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;

/**
 * Usa {@link Map} y {@linkplain List#of(Collection[]) listas}.
 *
 * @see Set
 * @throws IOException nunca
 */
public class Pmd {
    private Pmd() {
    }

    interface Shape {
        public static final int SIDES = 4;

        @Deprecated
        public abstract double area();

        public static class Inner {
        }

        public enum Kind { ROUND, SQUARE }

        public record Point(int x, int y) {
        }

        static interface Nested {
        }
    }

    public abstract interface Old {
        void run();
    }

    static interface Member {
    }

    static enum Color { RED, GREEN }

    enum Plain {
        ONE, TWO;

        private Plain() {
        }

        public final String lower() {
            return name();
        }

        static final int count() {
            return 2;
        }
    }

    enum WithBodies {
        ONE {
            @Override
            public final int value() {
                return 1;
            }
        };

        public final int code() {
            return 0;
        }

        public abstract int value();
    }

    static final record Pair(int left, int right) {
        public final int sum() {
            return left + right;
        }
    }

    static class Helper {
        public Helper() {
        }

        @SuppressWarnings("unchecked")
        private final
        void hidden() {
        }

        private static strictfp double half(double value) {
            return value / 2;
        }

        @SafeVarargs
        private final void many(List<String>... lists) {
        }

        void read() throws Exception {
            try (final StringReader reader = new StringReader("")) {
                reader.read();
            }
        }
    }

    final static class Sealed {
        final void locked() {
        }
    }

    abstract static class Base {
        abstract void run(final int times);
    }

    @Deprecated
    private static class OnlyPrivate {
        private OnlyPrivate() {
        }
    }

    private abstract static class AbstractEmpty {
    }

    private static class Extended {
        private Extended() {
        }
    }

    private static class Anonymous {
        private Anonymous() {
        }
    }

    private static class Generic<T> {
        private class Inner {
        }
    }

    private static class FromGeneric extends Generic<String> {
        private FromGeneric() {
        }
    }

    private class Child extends Generic<String>.Inner {
    }

    private static final class Subclass extends Extended {
    }

    @SuppressWarnings("all")
    private static class Silenced {
        private Silenced() {
        }
    }

    @SuppressWarnings("PMD.ClassWithOnlyPrivateConstructorsShouldBeFinal")
    private static class SilencedByName {
        private SilencedByName() {
        }
    }

    @SuppressWarnings("PMD")
    interface SilencedInterface {
        public void run();
    }

    void earlier() {
        class Parent {
        }
        class Kid extends Parent {
        }
    }

    private static class Parent {
        private Parent() {
        }
    }

    private static class Kid extends Parent {
    }

    private static class Shadowed {
        private Shadowed() {
        }
    }

    void shadows() {
        class Shadowed {
        }
        class FromLocal extends Shadowed {
        }
    }

    static Object make() {
        return true ? new Anonymous() { } : null;
    }
}
