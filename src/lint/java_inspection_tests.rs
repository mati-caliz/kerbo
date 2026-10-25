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
