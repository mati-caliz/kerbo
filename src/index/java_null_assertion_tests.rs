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
