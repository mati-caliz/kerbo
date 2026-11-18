package sample.inferred;

public final class Overloads {
    private Overloads() {
    }

    public static int size(String text) {
        return text.length();
    }

    public static int size(StringBuilder text) {
        return text == null ? 0 : text.length();
    }

    public static int all(String text) {
        return text.length();
    }

    public static int all(String... texts) {
        return texts == null ? 0 : texts.length;
    }

    public static int exported(String text) {
        return text.length();
    }

    static int exported(StringBuilder text) {
        return text == null ? 0 : text.length();
    }

    public static int any(Object value) {
        return value.hashCode();
    }

    public static int any(Object... values) {
        return values == null ? 0 : values.length;
    }
}
