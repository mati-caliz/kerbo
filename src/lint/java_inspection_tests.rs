use super::tests::{expected, lints, rendered_where, strict};
use crate::rules::{JavaRule, RuleId};

#[test]
fn a_private_field_assigned_only_once_may_be_final() {
    let source = "class A {
    private java.time.Instant createdAt = java.time.Instant.now();
    private int count = 0;
    private String name;
    private String label;
    @org.mockito.Spy
    private Object spy = new Object();
    private static int shared = 1;
    private final int done = 1;

    A(String name) {
        this.name = name;
        this.label = name;
    }

    A() {
        this(\"x\");
    }

    void bump() {
        count++;
        label = \"y\";
    }
}
";
    let found = rendered_where(&lints(source, &strict()), |rule| {
        rule == RuleId::Java(JavaRule::FieldMayBeFinal)
    });
    assert_eq!(
        found,
        expected(
            "2:31 field-may-be-final createdAt
4:20 field-may-be-final name
8:24 field-may-be-final shared"
        )
    );
}

// Sólo cuenta lo que ya tenía la variable en el mismo bloque: el valor por defecto de un arreglo de
// una dimensión, o el mismo literal recién asignado. Usarla en el medio lo olvida, y pasar el
// arreglo a otro lado deja de seguirlo.
#[test]
fn assigning_the_value_a_local_already_holds_is_reported() {
    let source = "class A {
    void f(int[] given) {
        final boolean[] flags = new boolean[2];
        flags[0] = true;
        flags[1] = false;
        final int[][] grid = new int[2][3];
        grid[1][2] = 0;
        grid[1][2] = 100;
        grid[1][2] = 100;
        int count = 0;
        count = 0;
        use(count);
        count = 0;
        final int[] shared = new int[2];
        Object alias = shared;
        shared[0] = 1;
        use(alias);
        shared[0] = 1;
        given[0] = 1;
        given[0] = 1;
        flags[3] = false;
        String[] names = new String[1];
        for (int i = 0; i < names.length; i++) {
            names[i] = \"x\";
        }
        names[0] = null;
    }

    void use(Object value) {
    }
}
";
    let found = rendered_where(&lints(source, &strict()), |rule| {
        rule == RuleId::Java(JavaRule::AlreadyAssigned)
    });
    assert_eq!(
        found,
        expected(
            "5:9 already-assigned flags[1]
9:9 already-assigned grid[1][2]
11:9 already-assigned count"
        )
    );
}
