package billing;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Set;
import java.util.stream.Collectors;

public final class Customers {
    private final List<String> names = new ArrayList<>();

    public void register(String name) {
        names.add(name);
    }

    public List<String> sorted() {
        List<String> copy = new ArrayList<>(names);
        Collections.sort(copy);
        return copy;
    }

    public boolean contains(String name, boolean ignoreCase) {
        for (String existing : names) {
            if (ignoreCase ? existing.equalsIgnoreCase(name) : existing.equals(name)) {
                return true;
            }
        }
        return false;
    }

    public static final class Names {
        private Names() {
        }

        public static String initials(String first, String last) {
            return first.substring(0, 1) + last.substring(0, 1);
        }
    }
}
