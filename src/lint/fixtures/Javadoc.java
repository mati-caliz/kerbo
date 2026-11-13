package fixtures;

import java.util.List;
/** bad import doc */
import java.util.Map;

/**
 * Type.
 * @apiNote hola
 * @implSpec chau
 * @param <T> t
 * @param <Q> nope
 * @param foo nope
 * @author me
 * @since 1
 * @custom x
 */
public class Javadoc<T, U> {
    private static final long serialVersionUID = 1L;
    int a, b;
    /** doc */ int c;
    int d;
    // line
    int e;
    /** doc */
    // line

    int f;
    /** doc */
    /* block */
    int g;
    @Deprecated
    int h;
    /** doc */
    @Deprecated
    int i;
    @Deprecated
    /** after annotation */
    int j;
    int z; /** trailing doc */
    int w;

    public Javadoc() { }

    /**
     * Doc.
     * @return
     */
    public int m1(int x, int y) { return 1; }

    /**
     * Doc.
     * @param x x
     * @param z z
     * @return value
     * @return again
     */
    public int m2(int x) { return 1; }

    /**
     * {@inheritDoc}
     */
    public int m3(int x) { return 1; }

    /**
     * Doc {@inheritDoc}
     * @param q q
     */
    public void m4(int x) { }

    /**
     * Doc.
     * @return
     *     the value
     */
    public int m5() { return 1; }

    /**
     * Doc.
     * @return x
     */
    public void m6() { }

    /** Doc. */
    @Override
    public String toString() { return ""; }

    @Override
    public int hashCode() { return 1; }

    /** Doc. */
    public <V> V m7(V v) { return v; }

    /**
     * Doc.
     * @throws Exception sometimes
     * @exception RuntimeException never
     * @see Object
     * @unknown z
     */
    public void m8() throws Exception {
        /** inside */
        int local = 1;
        Runnable r = new Runnable() {
            int anonField;
            /** run */
            public void run(int a) { }
        };
        /** local class */
        class Local {
            int localField;
            /** m */
            public int lm(int a) { return a; }
        }
        /** lambda */
        Runnable l = () -> { };
    }

    void packageMethod() { }
    protected void protectedMethod() { }
    private void privateMethod() { }
    /** d */ public void sameLine(int x) { }
    /** doc */
    // comment
    public void afterLineComment(int y) { }
    /** first */
    /** second */
    public void twoDocs(int q) { }
    /** @param q q */
    /* plain */
    public void docThenPlain(int q) { }
    /**/
    public void emptyComment() { }
    /***/
    public void tripleStar() { }
    /** doc */ public
    void split() { }
    public /** mid */ void mid() { }
    @Deprecated /** mid */ public void midAnn() { }
    /**
     * @param a
     * @param b b
     * @param c c c
     */
    public void params(int a, int b, int c) { }
    /**
     * Text @param a inline
     */
    public void inlineParam(int a) { }
    /** @param a a */
    public void oneLiner(int a) { }
    /** @return a */
    public int oneLinerRet() { return 1; }
    /**
     * @return*/
    public int retEnd() { return 1; }
    /**
     *@param a a
     *@return r
     */
    public int noSpace(int a) { return a; }
    /**
     * @param a a
     * @param a again
     */
    public void dup(int a) { }
    /**
     * {@inheritDoc}
     */
    public Javadoc(String text) { }
    /**
     * Doc.
     */
    public static void varargs(String... names) { }
    /**
     * Doc.
     * @param <S> s
     * @param <X> x
     */
    public <S> void typeTags() { }
    /**
     * Doc.
     */
    public int[] arr()[] { return null; }
    /**
     * @return
     */
    public abstract int abs(int a);
    /**
     * @return r
     * @param a a
     * @param b b
     */
    public int order(int a) { return 1; }
    /** static init */
    static { }
    /** instance init */
    { }
    /** doc */
    ;

    public interface I {
        int CONST = 1;
        void im();
        /** doc */
        void im2(int p);
        private void ip() { }
        default void id() { }
        class Nested { public void nm() { } }
    }

    public enum E {
        ONE, /** two */ TWO,
        /** three */
        THREE {
            int body;
            public void em() { }
            /** m */
            public int em2(int a) { return a; }
        };
        E() { }
        public void em() { }
        /** after semicolon */
    }

    public @interface Ann {
        int value();
        /** doc */
        String name();
        /**
         * doc
         * @return x
         */
        String other();
    }

    /** nested record */
    public record Nr(/** comp */ int a) {
        /** canon */
        public Nr(int a) { this.a = a; }
        /** m */
        public int rm(int b) { return b; }
    }
    /** generic class */
    static class G<T> { }
    @Deprecated
    /** after ann */
    static class J { }
    /**
     * Doc.
     * @param <T>
     */
    static class K<T> { }
    /**
     * Doc.
     * @param <T>t
     * @param T x
     */
    static class L<T> { }
    /**
     * @param  <T>  two spaces
     * @param <T> dup
     */
    static class M<T> { }
    /** Doc. */
    @javax.annotation.processing.Generated("x")
    static class N<T> { }
    /** Doc. */
    @Generated("x")
    static class O<T> { }
    /**
     * Doc.
     * @implNote x
     * @hidden x
     * @serial x
     * @uses x
     * @foo-bar x
     *@nospace x
     **  @double x
     * text @inline x
     * @since
     */
    static class P { }

    static class Inner { public void x() { } }

    Runnable field = new Runnable() { int anonInField; public void run() { } };

    /**
     * Varargs with an annotation before the dots.
     * @param first first
     * @param rest rest
     */
    public void annotatedVarargs(int first, Object @Deprecated ... rest) { }

    /**
     * Varargs with an annotation before the dots and no tag for it.
     */
    public void undocumentedVarargs(Object @Deprecated ... rest) { }

    /**
     * Snippet with annotations inside.
     *
     * {@snippet :
     * new Runnable() {
     *   @Override
     *   public void run() { }
     * }
     * }
     * @since 1
     */
    static class Snippet { }
    /** end */
}
/**
 * Record.
 * @param a a
 * @param zz no
 */
record R(int a, int b) {
    /** canon */
    public R(int a) { this(a, 0); }
}
/** trailing */
