use std::path::PathBuf;

use super::{GoPackageFile, drop_package_names};
use crate::lint::go::{CapitalizedError, GoPackageFacts};
use crate::lint::{LintFinding, LintKind};
use crate::rules::{Diagnostic, FileIdentity, GoRule, finding_diagnostics};

fn identity(relative: &str) -> FileIdentity {
    FileIdentity {
        display: relative.to_owned(),
        relative: relative.to_owned(),
        absolute: PathBuf::from(relative),
    }
}

fn capitalized(line: usize, word: &str) -> CapitalizedError {
    CapitalizedError {
        line,
        column: 1,
        word: word.to_owned(),
    }
}

fn diagnostics(relative: &str, subjects: &[(usize, &str)]) -> Vec<Diagnostic> {
    let found: Vec<LintFinding> = subjects
        .iter()
        .map(|(line, subject)| LintFinding {
            kind: LintKind::Go(GoRule::St1005),
            line: *line,
            column: 1,
            subject: (*subject).to_owned(),
            fix: None,
        })
        .collect();
    finding_diagnostics(&identity(relative), &found).collect()
}

fn file(relative: &str, package: &str, declared: &[&str], words: Vec<CapitalizedError>) -> GoPackageFile {
    GoPackageFile {
        relative: relative.to_owned(),
        facts: GoPackageFacts {
            package: package.to_owned(),
            declared: declared.iter().map(|name| (*name).to_owned()).collect(),
            capitalized: words,
            outside_functions: Vec::new(),
        },
    }
}

fn outside(relative: &str, position: (usize, usize)) -> GoPackageFile {
    let mut found = file(relative, "store", &[], Vec::new());
    found.facts.outside_functions.push(position);
    found
}

#[test]
fn a_capitalized_word_that_names_a_function_of_the_package_is_not_reported() {
    let files = [
        file(
            "store/load.go",
            "store",
            &["Load"],
            vec![capitalized(3, "Close"), capitalized(4, "Missing")],
        ),
        file("store/close.go", "store", &["Close"], Vec::new()),
        file("store/helpers_test.go", "store", &["Missing"], Vec::new()),
        file("store/load_test.go", "store", &[], vec![capitalized(5, "Bad")]),
        file("other/load.go", "other", &[], vec![capitalized(6, "Close")]),
        outside("store/vars_test.go", (8, 1)),
    ];
    let mut found = diagnostics(
        "store/load.go",
        &[(3, "Close"), (4, "Missing"), (3, "errors.New")],
    );
    found.extend(diagnostics("store/load_test.go", &[(5, "Bad")]));
    found.extend(diagnostics("other/load.go", &[(6, "Close")]));
    found.extend(diagnostics("store/vars_test.go", &[(8, "errors.New")]));
    drop_package_names(&mut found, &files);
    let kept: Vec<String> = found
        .iter()
        .map(|diagnostic| format!("{}:{}", diagnostic.relative_path, diagnostic.line))
        .collect();
    assert_eq!(
        kept,
        [
            "store/load.go:4",
            "store/load.go:3",
            "other/load.go:6",
            "store/vars_test.go:8"
        ]
    );
}
