package a.b;

public interface Mods {
  public static final int A = 1, B = 2;
  public abstract void m(final int x);
  static void s() {}
  public static class Nested { public Nested() {} }
  static interface In { }
  abstract interface In2 { }
  public enum En { X; private En() {} }
  static record Rec(int a) { public Rec { } public Rec(String s) { this(1); } }
  strictfp default void d() {}
}

final class FinalOwner {
  final void f() {}
  private final void g() {}
  @SafeVarargs final <T> void h(T... t) {}
  record Inner() { final void k() {} }
  static enum E2 { A { final void q() {} }; static final void w() {} final void z() {} }
  void anon() {
    Object o = new Object() { final void y() {} };
    try (final java.io.InputStream in = null) { } catch (final Exception _) { }
    for (final var _ : java.util.List.of(1)) { }
  }
}

class Package {
  public Package() {}
  public static class PubNested { public PubNested() {} }
  protected static class ProtNested { public ProtNested() {} }
  private static class PrivNested { public PrivNested() {} }
}

abstract class Abs {
  abstract void a(final int x);
  native void b(final int x);
}

@interface Note {
  public abstract int v() default 1;
  public static final int C = 1;
  public static class Inside { }
}
