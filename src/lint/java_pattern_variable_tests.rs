use super::tests::{lints, strict};
use crate::limits::{JavaSettings, Limits};
use crate::rules::{JavaRule, RuleId};

const SOURCE: &str = "import java.util.List;
import java.util.Map;

class Casts {
    void enclosing(Object value) {
        if (value instanceof String) {
            String text = (String) value;
        }
        if (value != null && value instanceof Integer) {
            Integer number = (Integer) value;
        }
        if (!(value instanceof Long)) {
            return;
        }
        Long amount = (Long) value;
    }

    void negatedElse(Object value) {
        if (!(value instanceof String)) {
            System.out.println();
        } else {
            String text = (String) value;
        }
    }

    void reassigned(Object value) {
        if (!(value instanceof String)) {
            return;
        }
        value = value.toString();
        String text = (String) value;
    }

    void others(Object value, String already) {
        String plain = (String) value;
        if (value instanceof Map<?, ?>) {
            Map<?, ?> map = (Map<?, ?>) value;
            Map<String, Object> typed = (Map<String, Object>) value;
        }
        if (value instanceof List) {
            Object raw = (List) value;
        }
        if (value instanceof String) {
            String same = (String) already;
        }
        while (value instanceof String) {
            String looped = (String) value;
            value = looped.trim();
        }
    }
}
";

fn reported(limits: &Limits) -> Vec<(usize, String)> {
    lints(SOURCE, limits)
        .into_iter()
        .filter(|finding| finding.kind.rule() == RuleId::Java(JavaRule::PatternVariableCanBeUsed))
        .map(|finding| (finding.line, finding.subject))
        .collect()
}

#[test]
fn reports_a_cast_that_an_instanceof_already_guarantees() {
    let expected: Vec<(usize, String)> = [
        (7, "text"),
        (10, "number"),
        (15, "amount"),
        (22, "text"),
        (37, "map"),
        (47, "looped"),
    ]
    .into_iter()
    .map(|(line, name)| (line, name.to_owned()))
    .collect();
    assert_eq!(reported(&strict()), expected);
}

#[test]
fn needs_release_sixteen() {
    let java11 = strict().with_java(JavaSettings {
        release: Some(11),
        ..JavaSettings::default()
    });
    assert_eq!(reported(&java11), []);
}

#[test]
fn reports_a_cast_where_the_instanceof_already_declares_a_pattern_variable() {
    let source = "import java.util.Collection;\n\nclass Merge<V> {\n    void merge(Collection<V> values, Object value) {\n        if (values instanceof Batch batch && batch.ready()) {\n            for (V each : ((Batch<V>) values).items()) {\n            }\n        }\n        if (value instanceof String text) {\n            try {\n                parse((String) value);\n            } catch (RuntimeException e) {\n            }\n        }\n        if (value instanceof Integer number && ((Integer) value) > 0) {\n        }\n        Object other = value instanceof String text ? ((String) value).trim() : null;\n    }\n\n    void parse(String text) {\n    }\n}\n";
    let found: Vec<(usize, String)> = lints(source, &strict())
        .into_iter()
        .filter(|finding| finding.kind.rule() == RuleId::Java(JavaRule::PatternVariableCanBeUsed))
        .map(|finding| (finding.line, finding.subject))
        .collect();
    let expected: Vec<(usize, String)> = [(6, "batch"), (11, "text"), (15, "number"), (17, "text")]
        .into_iter()
        .map(|(line, name)| (line, name.to_owned()))
        .collect();
    assert_eq!(found, expected);
}
