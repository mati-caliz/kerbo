use super::tests::{lints, strict};
use crate::limits::{JavaSettings, Limits};
use crate::rules::{JavaRule, RuleId};

const SOURCE: &str = "import java.util.*;

class Copies {
    private final List<String> names;
    private final Set<String> tags;

    Copies(Collection<String> source, Map<String, Integer> counts, int capacity, String maybe, java.util.concurrent.ConcurrentMap<String, Integer> shared) {
        names = Collections.unmodifiableList(new ArrayList<>(source));
        tags = Collections.unmodifiableSet(new HashSet<>(counts.keySet()));
        List<String> fixed = Collections.unmodifiableList(Arrays.asList(\"a\", \"b\"));
        Set<String> repeated = Collections.unmodifiableSet(new HashSet<>(Arrays.asList(\"a\", \"a\")));
        List<String> nullable = Collections.unmodifiableList(Arrays.asList(\"a\", maybe));
        Map<String, Integer> copy = Collections.unmodifiableMap(new HashMap<>(counts));
        List<String> sized = Collections.unmodifiableList(new ArrayList<>(capacity));
        List<String> own = Collections.unmodifiableList(new ArrayList<>(listed()));
        Map<String, Integer> concurrent = Collections.unmodifiableMap(new HashMap<>(shared));
    }

    List<String> listed() {
        return names;
    }
}
";

fn reported(limits: &Limits) -> Vec<(usize, String)> {
    lints(SOURCE, limits)
        .into_iter()
        .filter(|finding| finding.kind.rule() == RuleId::Java(JavaRule::Java9CollectionFactory))
        .map(|finding| (finding.line, finding.subject))
        .collect()
}

#[test]
fn reports_unmodifiable_copies_and_fixed_lists() {
    let expected: Vec<(usize, String)> = [
        (8, "List.copyOf"),
        (9, "Set.copyOf"),
        (10, "List.of"),
        (13, "Map.copyOf"),
        (15, "List.copyOf"),
    ]
    .into_iter()
    .map(|(line, factory)| (line, factory.to_owned()))
    .collect();
    assert_eq!(reported(&strict()), expected);
}

#[test]
fn copies_need_ten_and_fixed_lists_nine() {
    let java9 = strict().with_java(JavaSettings {
        release: Some(9),
        ..JavaSettings::default()
    });
    assert_eq!(reported(&java9), [(10, "List.of".to_owned())]);
}
