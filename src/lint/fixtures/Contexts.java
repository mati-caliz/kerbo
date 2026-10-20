package a.b;

import x.InExtends;
import x.InImplements;
import x.InNew;
import x.InTypeArgument;
import x.InCast;
import x.InParameter;
import x.InReturn;
import x.InLocal;
import x.InField;
import x.InInstanceof;
import x.InThrows;
import x.InBound;
import x.InArrayNew;
import x.InClassLiteral;
import x.InMethodTypeArgument;
import x.InCatch;
import x.InInterfaceExtends;
import x.InRecordImplements;
import x.InAnnotationArg;
import x.InWildcard;
import x.InArrayType;
import x.InVarargs;
import x.InGenericMethodRef;

public abstract class Contexts extends p.q.InExtends<T> implements p.q.InImplements<T> {
  p.q.InField<T> field;
  abstract p.q.InReturn<T> m(p.q.InParameter<T> a) throws p.q.InThrows;
  <U extends p.q.InBound<U>> void n(p.q.InVarargs<T>... v) {
    p.q.InLocal<T> local = new p.q.InNew<T>();
    java.util.List<p.q.InTypeArgument<T>> list;
    Object c = (p.q.InCast<T>) null;
    boolean i = c instanceof p.q.InInstanceof<?>;
    Object arr = new p.q.InArrayNew<?>[1];
    Object k = java.util.Collections.<p.q.InMethodTypeArgument<T>>emptyList();
    java.util.List<? extends p.q.InWildcard<T>> w;
    p.q.InArrayType<T>[] at;
    Object r = p.q.InGenericMethodRef<T>::new;
  }
  interface I extends p.q.InInterfaceExtends<T> {}
  record R() implements p.q.InRecordImplements<T> {}
}
