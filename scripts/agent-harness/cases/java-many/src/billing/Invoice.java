package billing;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.List;
import java.io.*;

public class Invoice {
    private final String customer;
    private final List<Line> lines = new ArrayList<>();

    public Invoice(String customer) {
        this.customer = customer;
    }

    public void add(Line line) {
        lines.add(line);
    }

    public long total() {
        long sum = 0;
        for (Line line : lines) {
            sum += line.amount();
        }
        return sum;
    }

    public String describe(String prefix, boolean upper) {
        String text = prefix + " " + customer;
        return upper ? text.toUpperCase() : text;
    }

    public interface Line {
        public long amount();

        public abstract String concept();
    }
}
