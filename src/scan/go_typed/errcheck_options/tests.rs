use super::findings;
use crate::config::GoTarget;
use crate::go_types::{AssertionFact, CallFact, FileFacts, RangeFact};
use crate::language::Language;
use crate::limits::Preset;
use crate::lint::go::platform::GoPlatform;

fn platform((check_blank, check_type_assertions): (bool, bool)) -> GoPlatform {
    GoPlatform::of(&GoTarget {
        errcheck_exclude_functions: vec!["os.Remove".to_owned()],
        errcheck_check_blank: check_blank,
        errcheck_check_type_assertions: check_type_assertions,
        ..GoTarget::default()
    })
}

fn call(name: &str) -> CallFact {
    CallFact {
        names: Some(vec![name.to_owned()]),
        function: name.to_owned(),
        ..CallFact::default()
    }
}

fn facts() -> FileFacts {
    FileFacts {
        blanks: Some(vec![
            RangeFact {
                line: 3,
                column: 2,
                call: Some(call("os.Open")),
            },
            RangeFact {
                line: 4,
                column: 2,
                call: Some(call("os.Remove")),
            },
            RangeFact {
                line: 5,
                column: 2,
                call: None,
            },
        ]),
        assertions: Some(vec![
            AssertionFact {
                line: 6,
                column: 7,
                blank: false,
            },
            AssertionFact {
                line: 7,
                column: 5,
                blank: true,
            },
        ]),
        ..FileFacts::default()
    }
}

fn found_lines(options: (bool, bool)) -> Vec<(usize, String)> {
    let limits = Preset::Strict.limits(Language::Go, false);
    findings(&facts(), &limits, &platform(options))
        .into_iter()
        .map(|finding| (finding.line, finding.subject))
        .collect()
}

#[test]
fn without_the_options_nothing_comes_out() {
    assert_eq!(found_lines((false, false)), []);
}

#[test]
fn check_blank_skips_the_excluded_calls() {
    assert_eq!(
        found_lines((true, false)),
        [(3, "os.Open".to_owned()), (5, String::new())]
    );
}

#[test]
fn an_assertion_with_a_blank_ok_also_needs_check_blank() {
    assert_eq!(found_lines((false, true)), [(6, String::new())]);
    assert_eq!(
        found_lines((true, true)),
        [
            (3, "os.Open".to_owned()),
            (5, String::new()),
            (6, String::new()),
            (7, String::new())
        ]
    );
}
