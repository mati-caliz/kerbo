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

public abstract class Two extends q.InExtends<T> implements q.InImplements<T> {
  q.InField<T> field;
  abstract q.InReturn<T> m(q.InParameter<T> a) throws q.InThrows;
  <U extends q.InBound<U>> void n(q.InVarargs<T>... v) {
    q.InLocal<T> local = new q.InNew<T>();
    java.util.List<q.InTypeArgument<T>> list;
    Object c = (q.InCast<T>) null;
    boolean i = c instanceof q.InInstanceof<?>;
    Object arr = new q.InArrayNew<?>[1];
    Object k = java.util.Collections.<q.InMethodTypeArgument<T>>emptyList();
    java.util.List<? extends q.InWildcard<T>> w;
    q.InArrayType<T>[] at;
    Object r = q.InGenericMethodRef<T>::new;
  }
  interface I extends q.InInterfaceExtends<T> {}
  record R() implements q.InRecordImplements<T> {}
}
