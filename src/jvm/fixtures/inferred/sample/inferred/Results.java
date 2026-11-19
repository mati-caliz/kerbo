package sample.inferred;

public final class Results {
    private Results() {
    }

    public static String orNull(Object value) {
        if (value == null || value.hashCode() == 0) {
            return null;
        }
        return value.toString();
    }

    public static String ofInstance(Object value) {
        if (value instanceof String) {
            return (String) value;
        }
        return null;
    }

    public static String text(Object value) {
        return String.valueOf(value);
    }

    public static String orEmpty(Object value) {
        return value == null ? "" : value.toString();
    }

    public static String guarded(Object value) {
        try {
            return value == null ? null : value.toString();
        } catch (IllegalStateException failure) {
            return "";
        }
    }
}
