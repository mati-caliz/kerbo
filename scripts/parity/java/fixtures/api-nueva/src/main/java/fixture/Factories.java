package fixture;

import java.util.ArrayList;
import java.util.Collections;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

class Factories {
    static final String KEY = "k";

    enum Day { MON, TUE }

    List<String> added() {
        List<String> l = new ArrayList<>();
        l.add("a");
        l.add("b");
        return Collections.unmodifiableList(l);
    }

    Set<String> repeated() {
        Set<String> s = new HashSet<>();
        s.add("a");
        s.add("a");
        return Collections.unmodifiableSet(s);
    }

    Map<String, Integer> put() {
        Map<String, Integer> m = new HashMap<>();
        m.put("a", 1);
        m.put("b", 2);
        return Collections.unmodifiableMap(m);
    }

    List<String> parameter(String x) {
        List<String> l = new ArrayList<>();
        l.add(x);
        return Collections.unmodifiableList(l);
    }

    List<String> interleaved() {
        List<String> l = new ArrayList<>();
        l.add("a");
        System.out.println();
        l.add("b");
        return Collections.unmodifiableList(l);
    }

    List<String> doubleBrace() {
        return Collections.unmodifiableList(new ArrayList<String>() {{
            add("a");
            add("b");
        }});
    }

    List<Day> enumConstants() {
        List<Day> l = new ArrayList<>();
        l.add(Day.MON);
        l.add(Day.TUE);
        return Collections.unmodifiableList(l);
    }

    List<String> constantField() {
        List<String> l = new ArrayList<>();
        l.add(KEY);
        return Collections.unmodifiableList(l);
    }

    List<String> usedAfter() {
        List<String> l = new ArrayList<>();
        l.add("a");
        List<String> r = Collections.unmodifiableList(l);
        l.add("c");
        return r;
    }

    List<String> sized() {
        List<String> l = new ArrayList<>(4);
        l.add("a");
        return Collections.unmodifiableList(l);
    }

    List<String> empty() {
        List<String> l = new ArrayList<>();
        return Collections.unmodifiableList(l);
    }
}
