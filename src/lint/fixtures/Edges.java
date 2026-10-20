package a.b;

import x.Entry;
import x.List;
import x.Var;
import x.Shadow;
import x.Lit;
import static x.Y.called;
import static x.Y.qualified;
import x.Ctor;
import x.Member;
import x.Ann;
import x.Gen;
import x.Lbl;
import x.Loc;
import x.Arr;
import x.TypeAnn;
import java.lang.String;
import java.lang.reflect.Method;
import java.lang.Thread.State;
import a.b.Same;
import a.b.c.Deeper;
import java.util.*;
import static java.util.Map.*;
import java.lang.*;
import x.Dup;
import x.Dup;
import x.Dup;
import static x.Y.twice;
import static x.Y.twice;
import x.Last;

@Ann
public class Edges {
  Map.Entry<K, V> e;
  java.util.List<String> q;
  int Var;
  class Shadow {}
  Object o = Lit.class;
  Object p = Ctor::new;
  Object r = foo.Member::bar;
  Object s = Collections.<Gen>emptyList();
  void m() {
    called();
    obj.qualified();
    Lbl: for (;;) { break Lbl; }
    class Loc {}
    Arr[] arr;
    java.util.@TypeAnn List<String> t;
    Dup d; Last l;
  }
  void n() { Loc x; Shadow y; }
}
