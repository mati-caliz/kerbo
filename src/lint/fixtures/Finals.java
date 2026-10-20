package a.b;

public class Finals {
  private Finals() {}

  @Deprecated
  private static class NoCtor {}

  static class OnlyPrivate {
    private OnlyPrivate() {}
  }

  static class Mixed {
    private Mixed() {}
    Mixed(int x) {}
  }

  static class Extended {
    private Extended() {}
  }

  static class Child extends Extended {}

  static class AnonBase {
    private AnonBase() {}
  }

  static class AssignedAnon {
    private AssignedAnon() {}
  }

  Object anon = new AnonBase() {};
  Object assigned;

  void m(final int a, int b, @Deprecated String c, String... rest) {
    assigned = new AssignedAnon() {};
    class Local {
      private Local() {}
    }
  }

  Finals(Finals this, int _) {}

  enum En {
    A;
    private En() {}
  }

  record Rec(int a) {
    private Rec(String s) { this(1); }
  }
}
