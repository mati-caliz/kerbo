use super::java_null_tests::findings_in;

const LOOPS: &str = r"package app;

import org.jspecify.annotations.Nullable;

final class Node {
    @Nullable Node next;
    @Nullable Node tail;
    int value;

    static void unlink(Node first, Node target) {
        for (Node node = first; ; node = node.next) {
            if (node == target) {
                return;
            }
        }
    }

    static int checked(Node first) {
        int total = 0;
        for (Node node = first; node != null; node = node.next) {
            total += node.value;
        }
        return total;
    }

    static void walk(Node first) {
        Node current = first;
        while (current.value > 0) {
            current = current.next;
        }
    }

    void append(Node added) {
        tail.next = added;
    }

    static void same(Node target, @Nullable Node candidate) {
        if (candidate == target) {
            candidate.value = 1;
        }
    }

    static void copied(Node first, Node stop) {
        for (Node node = first; node != stop; node = node.next) {
            accept(node);
        }
    }

    static void accept(Node node) {
        node.value = 0;
    }
}
";

// En la vuelta siguiente de un `for` o un `while`, lo que el avance o el cuerpo asignaron desde un
// campo `@Nullable` puede llegar null; no si la condición lo chequea. Asignar el campo de un campo
// `@Nullable` lo desreferencia, y `x == y` con un `y` que el flujo no da por posiblemente null deja
// presente a `x`. En la vuelta siguiente sólo cuentan las desreferencias: pasar la variable a un
// parámetro no nulo ni se marca ni la deja presente.
#[test]
fn the_next_iteration_sees_what_the_previous_one_assigned() {
    assert_eq!(findings_in(LOOPS, "null-argument"), Vec::<String>::new());
    assert_eq!(
        findings_in(LOOPS, "data-flow-issue"),
        ["11:42 node()", "28:16 current()", "34:9 tail()", "44:54 node()"]
    );
}

const NULL_LOCALS: &str = r"package app;

final class Locals {
    static Locals make() {
        return new Locals();
    }

    void run() {}

    static void tried() {
        Locals first = null;
        Locals second = null;
        try {
            first = make();
            second = make();
        } catch (RuntimeException e) {
            second = make();
        }
        first.run();
        second.run();
    }

    static void failed() {
        Locals found = null;
        try {
            found = make();
        } catch (RuntimeException e) {
            fail(e.getMessage());
        }
        found.run();
    }

    static void fail(String message) {
        throw new AssertionError(message);
    }

    static void branches(boolean flag) {
        Locals chosen = null;
        if (flag) {
            chosen = make();
        }
        if (flag) {
            chosen.run();
        }
    }
}
";

// Una local que arranca en `null` y se asigna en un `try` puede seguir en null en el `catch`, y de
// ahí a lo que viene después. Fuera de ese camino el `null` inicial no se sigue: una condición que
// lo correlaciona con otra variable no se entiende.
#[test]
fn a_local_assigned_null_is_possibly_null_where_no_branch_reassigned_it() {
    assert_eq!(findings_in(NULL_LOCALS, "data-flow-issue"), ["19:15 run()"]);
}
