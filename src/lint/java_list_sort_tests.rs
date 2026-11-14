use super::tests::{lints, strict};
use crate::fix::apply;
use crate::limits::{JavaSettings, Limits};
use crate::rules::{JavaRule, RuleId};

const QUALIFIED: &str = "import java.util.Collections;
import java.util.List;

class Sorter {
    void run(List<String> names, List<String> others, boolean first) {
        Collections.sort(names, String.CASE_INSENSITIVE_ORDER);
        Collections.sort(names);
        java.util.Collections.sort(first ? names : others, null);
    }
}
";
const STATIC: &str = "import static java.util.Collections.sort;
import java.util.List;

class Sorter {
    void run(List<String> names) {
        sort(names, null);
    }
}
";
const SHADOWED: &str = "import static java.util.Collections.*;
import java.util.List;

class Sorter {
    void run(List<String> names) {
        sort(names, null);
    }

    static void sort(List<String> names, Object order) {
    }
}
";

const BOTH_IMPORTED: &str = "import static java.util.Arrays.sort;
import static java.util.Collections.sort;
import java.util.List;

class Sorter {
    void run(List<String> names, String[] array) {
        sort(array, null);
        sort(names.toArray(new String[0]), null);
        sort(names, null);
    }
}
";

fn sorts(source: &str, limits: &Limits) -> Vec<(usize, String)> {
    lints(source, limits)
        .into_iter()
        .filter(|finding| finding.kind.rule() == RuleId::Java(JavaRule::Java8ListSort))
        .map(|finding| (finding.line, finding.subject))
        .collect()
}

fn fixed(source: &str) -> String {
    lints(source, &strict())
        .into_iter()
        .filter(|finding| finding.kind.rule() == RuleId::Java(JavaRule::Java8ListSort))
        .filter_map(|finding| finding.fix)
        .rev()
        .try_fold(source.to_owned(), |text, fix| apply(&fix.edits, &text))
        .expect("los arreglos caen dentro del archivo")
}

#[test]
fn reports_collections_sort_with_a_comparator() {
    assert_eq!(
        sorts(QUALIFIED, &strict()),
        [(6, "names".to_owned()), (8, "first ? names : others".to_owned())]
    );
    assert_eq!(sorts(STATIC, &strict()), [(6, "names".to_owned())]);
    assert_eq!(sorts(SHADOWED, &strict()), []);
    assert_eq!(sorts(BOTH_IMPORTED, &strict()), [(9, "names".to_owned())]);
}

#[test]
fn needs_release_eight() {
    let java7 = strict().with_java(JavaSettings {
        release: Some(7),
        ..JavaSettings::default()
    });
    assert_eq!(sorts(QUALIFIED, &java7), []);
}

#[test]
fn the_fix_sorts_the_list_itself() {
    let qualified = fixed(QUALIFIED);
    assert!(qualified.contains("names.sort(String.CASE_INSENSITIVE_ORDER);"));
    assert!(qualified.contains("(first ? names : others).sort(null);"));
    assert!(fixed(STATIC).contains("names.sort(null);"));
}
