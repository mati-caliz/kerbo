package billing;

import java.math.BigDecimal;
import java.math.RoundingMode;
import java.util.*;
import java.util.Optional;

public class Taxes {
    private static final long WHOLE = 100;

    private Taxes() {
    }

    public static long withTax(long amount, long percent) {
        return amount + amount * percent / WHOLE;
    }

    public static BigDecimal rounded(BigDecimal value, int scale) {
        return value.setScale(scale, RoundingMode.HALF_UP);
    }

    public static String label(String name, long amount) {
        return name + ": " + amount;
    }

    public static boolean exempt(String category) {
        return category.isEmpty();
    }
}
