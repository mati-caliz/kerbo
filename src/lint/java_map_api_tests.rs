use super::tests::{lints, strict};
use crate::rules::{JavaRule, RuleId};

const SOURCE: &str = "import java.util.HashMap;
import java.util.List;
import java.util.Map;

class Cache {
    private final Map<Class<?>, Class<?>[]> classCache = new HashMap<>();
    private final List<String> names = null;

    Class<?>[] declared(final Class<?> type) {
        Class<?>[] result = classCache.get(type);
        if (result == null) {
            result = new Class[] { type };
            classCache.put(type, result);
        }
        return result;
    }

    Class<?>[] inline(final Class<?> type) {
        Class<?>[] result;
        result = classCache.get(type);
        if (null == result) {
            classCache.put(type, result = new Class[0]);
        }
        return result;
    }

    Class<?>[] calls(final Class<?> type) {
        Class<?>[] result = classCache.get(type);
        if (result == null) {
            result = build(type);
            classCache.put(type, result);
        }
        return result;
    }

    Class<?>[] otherKey(final Class<?> type, final Class<?> other) {
        Class<?>[] result = classCache.get(type);
        if (result == null) {
            result = new Class[] { type };
            classCache.put(other, result);
        }
        return result;
    }

    Class<?>[] reassigned(Class<?> type) {
        type = type.getSuperclass();
        Class<?>[] result = classCache.get(type);
        if (result == null) {
            result = new Class[] { type };
            classCache.put(type, result);
        }
        return result;
    }

    String notAMap(int index) {
        String name = names.get(index);
        if (name == null) {
            name = \"\";
            names.add(name);
        }
        return name;
    }

    Class<?>[] build(Class<?> type) {
        return new Class[] { type };
    }
}
";

fn lines() -> Vec<usize> {
    lints(SOURCE, &strict())
        .into_iter()
        .filter(|finding| finding.kind.rule() == RuleId::Java(JavaRule::Java8MapApi))
        .map(|finding| finding.line)
        .collect()
}

#[test]
fn a_get_null_check_and_put_is_reported_when_the_value_has_no_side_effects() {
    assert_eq!(lines(), [11, 21]);
}
