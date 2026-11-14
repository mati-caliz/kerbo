use super::tests::{lints, strict};
use crate::fix::apply;
use crate::limits::{JavaSettings, Limits};
use crate::rules::{JavaRule, RuleId};

const SOURCE: &str = "import java.util.List;
import java.util.Map;

class Sizes {
    static final int NONE = 0;

    void run(List<String> names, Map<String, Integer> totals, String text, StringBuilder buffer, Custom custom) {
        final int one = 1;
        boolean a = names.size() == 0;
        boolean b = 0 < totals.size();
        boolean c = text.length() >= one;
        boolean d = names.size() < 1;
        boolean e = names.size() > 1;
        boolean f = custom.size() == 0;
        boolean g = buffer.length() != 0;
        boolean h = names.size() == (0);
    }
}
";

fn replacements(limits: &Limits) -> Vec<(usize, String)> {
    lints(SOURCE, limits)
        .into_iter()
        .filter(|finding| finding.kind.rule() == RuleId::Java(JavaRule::SizeReplaceableByIsEmpty))
        .map(|finding| (finding.line, finding.subject))
        .collect()
}

#[test]
fn reports_size_comparisons_that_ask_for_emptiness() {
    let expected: Vec<(usize, String)> = [
        (9, "names.isEmpty()"),
        (10, "!totals.isEmpty()"),
        (11, "!text.isEmpty()"),
        (12, "names.isEmpty()"),
        (15, "!buffer.isEmpty()"),
        (16, "names.isEmpty()"),
    ]
    .into_iter()
    .map(|(line, replacement)| (line, replacement.to_owned()))
    .collect();
    assert_eq!(replacements(&strict()), expected);
}

#[test]
fn char_sequences_need_release_fifteen() {
    let java8 = strict().with_java(JavaSettings {
        release: Some(8),
        ..JavaSettings::default()
    });
    assert!(
        !replacements(&java8)
            .iter()
            .any(|(_, replacement)| replacement.contains("buffer"))
    );
}

#[test]
fn the_fix_writes_is_empty() {
    let fixed = lints(SOURCE, &strict())
        .into_iter()
        .filter(|finding| finding.kind.rule() == RuleId::Java(JavaRule::SizeReplaceableByIsEmpty))
        .filter_map(|finding| finding.fix)
        .rev()
        .try_fold(SOURCE.to_owned(), |text, fix| apply(&fix.edits, &text))
        .expect("los arreglos caen dentro del archivo");
    assert!(fixed.contains("boolean a = names.isEmpty();"));
    assert!(fixed.contains("boolean b = !totals.isEmpty();"));
}

#[test]
fn strings_need_release_six() {
    let java5 = strict().with_java(JavaSettings {
        release: Some(5),
        ..JavaSettings::default()
    });
    assert!(
        !replacements(&java5)
            .iter()
            .any(|(_, replacement)| replacement.contains("text"))
    );
}
