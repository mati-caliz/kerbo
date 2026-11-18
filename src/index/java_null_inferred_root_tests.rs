use super::java_null_tests::findings_in;

const ROOTS: &str = r#"package app;

import org.jspecify.annotations.Nullable;

final class Rule {
    Hub hub = new Hub();

    Node createNode() {
        return new Node();
    }
}

final class Hub {
    @Nullable Node find(String name) {
        return null;
    }

    <T> @Nullable T typed(String name, Class<T> type) {
        return null;
    }
}

class Node {
    @Nullable Node toComputer() {
        return null;
    }

    String url() {
        return "";
    }

    void use() {
    }
}

final class Web {
    Web(String url, int mode) {
    }

    static String crumbed(String url) {
        return url;
    }
}

interface Step {
    void run(Rule rule);
}

final class Roots {
    Rule j = new Rule();

    void then(Step step) {
    }

    void localClass() {
        Node c = j.createNode().toComputer();
        class A extends Node {}

        c.use();
    }

    void nestedArguments() {
        Node computer = j.createNode().toComputer();
        Hub hub = j.hub;
        hub.find("x");
        Web req = new Web(Web.crumbed(String.format("%s/config.xml", computer.url())), 1);
    }

    void inferredTypes() {
        var agent = j.createNode();
        var computer = agent.toComputer();
        computer.use();
    }

    void reassigned() {
        Node project = j.createNode();
        project.use();
        project = j.hub.typed("a", Node.class);
        System.out.println(project.url());
    }

    void lambdaParameter() {
        then(r -> {
            Node p = r.hub.typed("p", Node.class);
            p.use();
            var node = r.hub.find("n");
            node.use();
        });
    }

    void castAssignment() {
        Node s = (Node) j.hub.find("remote");
        s.use();
    }
}
"#;

// Una variable sin tipo escrito también es raíz de una cadena: un `var` toma el tipo de lo que lo
// inicializa (`agent.toComputer()` con `var agent = j.createNode()`), y el parámetro de una lambda,
// el del parámetro de la interfaz funcional que recibe la llamada (`then(Step)`, `Step.run(Rule)`),
// también cuando se le leen campos (`r.hub`). Una clase local, un `String.format` anidado, una
// reasignación o un casteo en el medio no cortan el flujo.
#[test]
fn variables_without_a_written_type_are_chain_roots() {
    assert_eq!(
        findings_in(ROOTS, "data-flow-issue"),
        [
            "59:11 use()",
            "66:79 url()",
            "72:18 use()",
            "79:36 url()",
            "85:15 use()",
            "87:18 use()",
            "93:11 use()"
        ]
    );
}
