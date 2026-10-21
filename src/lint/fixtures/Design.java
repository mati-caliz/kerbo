package probe;

import java.util.List;

public class Design {
    public int open;
    protected int shared;
    int packaged;
    private int count;
    public static final int MAX = 1;
    public final int frozen = 2;
    public static int global;
    private static final long serialVersionUID = 1L;
    public long serialPersistent;

    public Design(int count) {
        this.count = count;
    }

    private static int late;

    public void setCount(int count) {
        this.count = count;
    }

    public int compute(int open) {
        final int Total = open + 1;
        final int good = 2;
        int count = 3;
        return Total + good + count;
    }

    protected void hook() {
        System.out.println("x");
    }

    public void emptyMethod() {
    }

    public final void sealed() {
        System.out.println("y");
    }

    /**
     * Documented for extension.
     */
    public void documented() {
        System.out.println("z");
    }

    @org.junit.Test
    public void annotated() {
        System.out.println("t");
    }

    private void privateOne() {
        System.out.println("p");
    }

    public static void staticOne() {
        System.out.println("s");
    }

    public abstract static class Inner {
        public abstract void abstractOne();
        public void concrete() { System.out.println("c"); }
    }

    final class FinalInner {
        public void notOverridable() { System.out.println("f"); }
    }

    public int field2;

    java.util.List<String> qualified() { return null; }

    /** Not seen before type parameters. */
    <T> T typed() { return null; }
}

interface Shape {
    void draw();
    int SIDES = 4;
}

enum Kind {
    A;
    public void inEnum() { System.out.println("e"); }
}
