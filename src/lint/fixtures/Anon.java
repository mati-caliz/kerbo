package a.b;

import java.util.function.Supplier;

public class Anon {
  static class InLambda { private InLambda() {} }
  static class InTernary { private InTernary() {} }
  static class InCast { private InCast() {} }
  static class InParens { private InParens() {} }
  static class InChain { private InChain() {} Object go() { return null; } }
  static class InField { private InField() {} Object f; }
  static class InReturn { private InReturn() {} }
  static class InArray { private InArray() {} }
  static class InThrow extends RuntimeException { private InThrow() {} }
  static class InSwitch { private InSwitch() {} }
  static class InThisField { private InThisField() {} }
  static class InConcat { private InConcat() {} }
  static class InArg { private InArg() {} }
  static class Generic<T> { private Generic() {} }
  static class Qualified { private Qualified() {} }

  Object field;
  boolean flag;

  Object all(int k) {
    Supplier<Object> s = () -> new InLambda() {};
    Object t = flag ? new InTernary() {} : null;
    Object c = (Object) new InCast() {};
    Object p = (new InParens() {});
    Object ch = new InChain() {}.go();
    Object fi = new InField() {}.f;
    Object[] arr = { new InArray() {} };
    this.field = new InThisField() {};
    String cc = "" + new InConcat() {};
    use(new InArg() {});
    Object g = new Generic<String>() {};
    Object q = new Anon.Qualified() {};
    Object sw = switch (k) { case 1 -> new InSwitch() {}; default -> null; };
    if (k == 2) { throw new InThrow() {}; }
    return new InReturn() {};
  }

  void use(Object o) {}
}
