use super::tests::{lints, strict};
use crate::fix::apply;
use crate::limits::{JavaSettings, Limits};
use crate::rules::{JavaRule, RuleId};

const SOURCE: &str = "import java.util.*;

class Orders {
    private final List<String> names = new ArrayList<>();
    static final int FIRST = 0;

    String run(Deque<String> queue, TreeSet<String> sorted, Set<String> plain, String[] array) {
        String first = names.get(0);
        String last = this.names.get(names.size() - 1);
        names.remove(0);
        names.add(0, \"x\");
        String head = queue.iterator().next();
        String lowest = sorted.iterator().next();
        String any = plain.iterator().next();
        String constant = names.get(FIRST);
        String other = names.get(1);
        return array[0];
    }

    void series(List<String> parts) {
        String a = parts.get(0);
        String b = parts.get(1);
        String c = parts.get(2);
    }

    void separated(List<String> parts) {
        String a = parts.get(0);
        int x = 1;
        int y = 2;
        String b = parts.get(1);
    }

    static class Holder {
        List<String> items;
    }

    String nested(Holder holder) {
        return holder.items.get(0) + found().get(0);
    }

    private List<String> found() {
        return names;
    }
}
";

fn sequenced(limits: &Limits) -> Vec<(usize, String)> {
    lints(SOURCE, limits)
        .into_iter()
        .filter(|finding| finding.kind.rule() == RuleId::Java(JavaRule::SequencedCollectionMethod))
        .map(|finding| (finding.line, finding.subject))
        .collect()
}

#[test]
fn reports_index_calls_that_have_a_sequenced_method() {
    let expected: Vec<(usize, String)> = [
        (8, "getFirst"),
        (9, "getLast"),
        (10, "removeFirst"),
        (11, "addFirst"),
        (12, "getFirst"),
        (13, "getFirst"),
        (27, "getFirst"),
        (38, "getFirst"),
        (38, "getFirst"),
    ]
    .into_iter()
    .map(|(line, method)| (line, method.to_owned()))
    .collect();
    assert_eq!(sequenced(&strict()), expected);
}

#[test]
fn needs_release_twenty_one() {
    let java17 = strict().with_java(JavaSettings {
        release: Some(17),
        ..JavaSettings::default()
    });
    assert_eq!(sequenced(&java17), []);
}

#[test]
fn the_fix_calls_the_sequenced_method() {
    let fixed = lints(SOURCE, &strict())
        .into_iter()
        .filter(|finding| finding.kind.rule() == RuleId::Java(JavaRule::SequencedCollectionMethod))
        .filter_map(|finding| finding.fix)
        .rev()
        .try_fold(SOURCE.to_owned(), |text, fix| apply(&fix.edits, &text))
        .expect("los arreglos caen dentro del archivo");
    assert!(fixed.contains("String first = names.getFirst();"));
    assert!(fixed.contains("String last = this.names.getLast();"));
    assert!(fixed.contains("names.removeFirst();"));
    assert!(fixed.contains("names.addFirst(\"x\");"));
    assert!(fixed.contains("String head = queue.getFirst();"));
}
