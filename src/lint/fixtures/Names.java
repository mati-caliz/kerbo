package probe;

import java.io.Serializable;

public class Probe implements Serializable, Runnable {
    private static final long serialVersionUID = 1l;
    public static final String okConstant = "x";
    static final int BAD_lower = 2;
    private static int Counter = 0;
    private int Member_field;
    private final int finalMember = 3;
    int okField;

    interface inner_iface {
        int notConst = 1;
        void Do_thing(int Param);
    }

    enum colors { RED, green }

    record point(int X, int y) {}

    @interface marker {}

    public void Probe() {
        ;
        long a = 10l;
        for (int i = 0; i < 3; i++);
    }

    @Override
    public void run() {}

    @Override
    public String ToString() { return ""; }

    public Probe(int Bad_param, final int okParam) {
        int Local = 1;
        try { } catch (Exception Bad_e) { }
        java.util.function.IntUnaryOperator op = (int Lambda_p) -> Lambda_p;
        if (Local > 0) { ; }
    }

    void m(String... Values) {}

    class Nested {}
    class nested_bad {}
}
