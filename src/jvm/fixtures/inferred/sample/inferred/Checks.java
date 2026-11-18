package sample.inferred;

import java.util.List;

public class Checks {
    private final String kept;

    public Checks(String name) {
        this.kept = name.trim();
    }

    public static int direct(String text) {
        return text.length();
    }

    public static String untouched(String text) {
        return text;
    }

    public static void guarded(Object value) {
        if (value == null) {
            throw new IllegalArgumentException("value");
        }
    }

    public static int tolerant(String text) {
        if (text == null) {
            return 0;
        }
        return text.length();
    }

    public static int forwarded(String text) {
        return direct(text);
    }

    public static int branchedForward(String text, boolean flag) {
        if (flag) {
            return direct(text);
        }
        return text.length();
    }

    public static int oneBranch(String text, boolean flag) {
        if (flag) {
            return text.length();
        }
        return 0;
    }

    public int overridable(String text) {
        return text.length();
    }

    public static int throughVirtual(Checks checks, String text) {
        return checks.overridable(text);
    }

    public final int sealedLength(String text) {
        return text.length();
    }

    public static int throughFinal(Checks checks, String text) {
        return checks.sealedLength(text);
    }

    public static int caught(String text) {
        try {
            return text.length();
        } catch (RuntimeException e) {
            return 0;
        }
    }

    public static int cleaned(String text) {
        try {
            return text.length();
        } finally {
            System.gc();
        }
    }

    public static int looped(String[] items) {
        int total = 0;
        for (String item : items) {
            total += item.length();
        }
        return total;
    }

    public static int afterLoop(String text, int times) {
        int total = 0;
        for (int i = 0; i < times; i++) {
            total += i;
        }
        return total + text.length();
    }

    public static int typed(Object value) {
        if (value instanceof String) {
            return 1;
        }
        throw new IllegalStateException();
    }

    public static int wide(long first, double second, String text) {
        return text.length();
    }

    public static int listed(List<String> names) {
        return names.size();
    }

    public static int passedToInterface(List<String> names, String name) {
        return names.indexOf(name);
    }

    public static int number(int count) {
        return count;
    }

    public static int spin(String text) {
        return spin(text);
    }

    public static int switched(String text, int choice) {
        switch (choice) {
            case 1:
                return text.length();
            case 2:
                return text.hashCode();
            default:
                return text.isEmpty() ? 1 : 0;
        }
    }

    public static void stored(String[] target, String text) {
        target[0] = text;
    }

    public static int synchronizedOn(Object lock) {
        synchronized (lock) {
            return 1;
        }
    }

    public static long arithmetic(long base, int shift, String text) {
        long moved = (base << shift) + (long) shift * 2L;
        double scaled = moved / 3.0d;
        int[] cells = new int[] {shift, (int) scaled};
        cells[0] += 1;
        long[][] grid = new long[2][3];
        grid[0][0] = moved ^ base;
        return grid[0][0] + cells[1] + text.length();
    }

    public String kept() {
        return kept;
    }
}
