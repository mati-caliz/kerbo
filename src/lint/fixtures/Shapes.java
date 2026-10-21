package probe;

import sun.misc.Unsafe;
import java.util.List;

public class Second {
    final static public int ORDER = 1;
    @Deprecated public static void annotated() {}
    public @Deprecated void late() {}
    int a, b;
    int[] good; int bad[];
    String names()[] { return null; }

    void flow(int x, boolean flag) {
        switch (x) {
            case 1: break;
        }
        switch (x) { case 1: break; default: break; }
        if (flag) x++;
        if (flag) { } else x--;
        while (flag) flag = false;
        for (;;) { break; }
        { int nested = 1; }
        int y = 0;
        if ((y = x) > 0) { y++; }
        boolean same = flag == true;
        boolean neg = !false;
        try { } catch (Exception e) { }
        // TODO: limpiar
        /* TODO: otro */
	int tabbed = 1;   
    }

    @Override public boolean equals(Object other) { return false; }
}

interface Constants { int VALUE = 1; }

class Utility {
    public static int helper() { return 1; }
}

class WithHash {
    public int hashCode() { return 1; }
}