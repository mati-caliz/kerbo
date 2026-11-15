use super::java_null_tests::findings_in;

const SOURCE: &str = r#"package app;

import org.jspecify.annotations.Nullable;

final class Job {
    @Nullable Build getLastBuild() {
        return null;
    }

    static @Nullable Build find(String id) {
        return null;
    }
}

final class Build {
    int number;

    int getNumber() {
        return number;
    }

    boolean isDone() {
        return true;
    }

    Object workspace() {
        return this;
    }
}

final class Uses {
    static void hamcrest(String id) {
        Build found = Job.find(id);
        assertThat(found, notNullValue());
        found.getNumber();
        Build other = Job.find(id);
        assertThat("motivo", other, is(not(nullValue())));
        other.getNumber();
    }

    static void cachedGetter(Job job) {
        Build first = job.getLastBuild();
        first.getNumber();
        Build second = job.getLastBuild();
        second.getNumber();
    }

    static int chained(Job job) {
        Build last = job.getLastBuild();
        last.getNumber();
        return job.getLastBuild().getNumber();
    }

    static void correlated(Job job, int size) {
        Build parent = job.getLastBuild();
        boolean collapsed = size == 1 && parent != null;
        if (collapsed) {
            parent.getNumber();
        }
        Build build = Job.find("a");
        Object workspace = build != null ? build.workspace() : null;
        if (workspace != null) {
            build.getNumber();
        }
    }

    static void reference(String id) {
        Build build = Job.find(id);
        await(build::isDone);
        build.getNumber();
    }

    @SuppressWarnings("ConstantConditions")
    static void suppressed(String id) {
        Job.find(id).getNumber();
    }

    static void assertThat(Object value, Object matcher) {}

    static void assertThat(String reason, Object value, Object matcher) {}

    static Object notNullValue() {
        return new Object();
    }

    static Object nullValue() {
        return new Object();
    }

    static Object not(Object matcher) {
        return matcher;
    }

    static Object is(Object matcher) {
        return matcher;
    }

    static void await(java.util.function.BooleanSupplier condition) {}
}
"#;

// `assertThat(x, notNullValue())` de Hamcrest deja a `x` presente; un getter sin argumentos ya
// desreferenciado por una variable da lo mismo más adelante, también en una cadena; `x::m` desreferencia a `x` (y se marca
// ahí, no después); y `@SuppressWarnings("ConstantConditions")` apaga DataFlowIssue. Una `boolean`
// local guarda su chequeo, y `ws = b != null ? b.x() : null` hace que `ws` no nulo deje presente a `b`.
#[test]
fn assertions_cached_getters_references_and_suppressions_follow_intellij() {
    assert_eq!(
        findings_in(SOURCE, "data-flow-issue"),
        ["43:15 getNumber()", "50:14 getNumber()", "69:15 build()"]
    );
}

const FAILING_GUARDS: &str = r"package app;

final class Names {
    static void checkGoodName(String name) {
        if (name == null || name.isEmpty()) {
            throw new IllegalArgumentException();
        }
    }

    static String find(String id) {
        if (id.isEmpty()) {
            return null;
        }
        return id;
    }

    static boolean rename(String id) {
        String proposed = find(id);
        checkGoodName(proposed);
        return proposed.equals(id);
    }
}
";

// Pasar un valor a un método que siempre falla con null en ese lugar (cláusula `fail` inferida de su
// cuerpo) lo deja presente después, aunque el parámetro no sea no nulo.
#[test]
fn a_call_that_fails_on_null_leaves_its_argument_present() {
    assert_eq!(
        findings_in(FAILING_GUARDS, "data-flow-issue"),
        Vec::<String>::new()
    );
}

const PASSED_TWICE: &str = r"package app;

import java.util.List;

final class Client {
    Client withToken(String user) {
        user.length();
        return this;
    }
}

final class Rule {
    Client createClient() {
        return new Client();
    }
}

final class Queue {
    static String find(String id) {
        if (id.isEmpty()) {
            return null;
        }
        return id;
    }

    static void check(Rule rule, List<String> items) {
        String bob = find(rule.toString());
        Client client = rule.createClient();
        client.withToken(bob);
        for (String item : items) {
            if (item.isEmpty()) {
                item.hashCode();
            }
        }
        client = rule.createClient();
        rule.createClient().withToken(bob);
    }
}
";

// Un valor ya pasado a un parámetro no nulo no se vuelve a marcar al pasarlo de nuevo, aunque la
// segunda llamada sea sobre el resultado de otra.
#[test]
fn a_value_already_passed_to_a_non_null_parameter_is_not_reported_again() {
    assert_eq!(findings_in(PASSED_TWICE, "null-argument"), ["29:26 user"]);
}

const OWN_ANNOTATION: &str = r"package app;

import org.example.annotations.Nullable;

interface Node {
    @Nullable
    Node child(String name);

    String value();
}

final class Reader {
    static String read(Node node) {
        return node.child(node.value()).value();
    }
}
";

const KNOWN_ANNOTATION: &str = r"package app;

import org.jspecify.annotations.Nullable;

interface Node {
    @Nullable
    Node child(String name);

    String value();
}

final class Reader {
    static String read(Node node) {
        return node.child(node.value()).value();
    }
}
";

// IntelliJ sólo reconoce las anotaciones de nulidad de paquetes conocidos: una propia que se llama
// `Nullable` (la de maven, `org.apache.maven.api.annotations`) no dice nada.
#[test]
fn only_known_nullness_annotations_count() {
    assert_eq!(
        findings_in(OWN_ANNOTATION, "data-flow-issue"),
        Vec::<String>::new()
    );
    assert_eq!(
        findings_in(KNOWN_ANNOTATION, "data-flow-issue"),
        ["14:41 value()"]
    );
}

const REMEMBERED_CALLS: &str = r"package app;

final class Registry {
    static Registry instance;

    static Registry find() {
        return instance;
    }

    Registry lookup() {
        return null;
    }
}

final class Agent {
    Registry registry;

    Registry toRegistry() {
        Registry found = Registry.find();
        return found == null ? null : found.lookup();
    }

    Registry suffix() {
        return registry == null ? null : registry;
    }

    static void uses(Agent agent) {
        agent.toRegistry().lookup();
        agent.toRegistry().lookup();
        agent.suffix().lookup();
        agent.suffix().lookup();
    }
}
";

// Una llamada sin argumentos ya desreferenciada se recuerda si el método es puro (sólo llama métodos
// propios, no crea ni asigna: `domain.publicSuffix()` en guava); una impura se vuelve a marcar
// (`slave.toComputer()` en jenkins).
#[test]
fn only_pure_calls_are_remembered_after_a_dereference() {
    assert_eq!(
        findings_in(REMEMBERED_CALLS, "data-flow-issue"),
        ["28:28 lookup()", "29:28 lookup()", "30:24 lookup()"]
    );
}
