package a.b;

import com.example.Deserializer;
import com.example.Mapper;
import java.util.List;

public abstract class Quirks extends com.example.Deserializer<Object> {
  java.util.Map.Entry<String, Mapper> entry;

  public static void check(
      boolean expression,
      /*
       * Un comentario con comas, paréntesis (y) y apóstrofes: no parte la lista.
       */
      @Nullable String template,
      @Nullable Object @Nullable ... arguments) {}

  void annotated(final @Nullable Object @Nullable ... rest) {}

  private static class Red<A> {
    private class Orange {}

    private class Yellow<B> extends Red<B>.Orange {}
  }

  private static class From<K> {
    class To<V> {}
  }

  void qualified() {
    Object first = new From<Integer>() {}.new To<String>();
    Object second = new From<Integer>().new To<String>() {};
  }
}
