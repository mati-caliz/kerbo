use std::fs;

use super::super::{EXIT_CLEAN, EXIT_FAILURE, EXIT_FINDINGS, Terminal, run};
use crate::cli::try_parse;
use crate::i18n::{Lang, LangPreference};
use crate::test_support::TempTree;

const UNFORMATTED: &str = "package main\nfunc main(){\nprintln(1+2)\n}\n";
const FORMATTED: &str = "package main\n\nfunc main() {\n\tprintln(1 + 2)\n}\n";

struct Outcome {
    status: u8,
    report: String,
}

fn fmt(tree: &TempTree, flags: &[&str]) -> Outcome {
    let path = tree.path().to_str().expect("la ruta es UTF-8");
    let mut arguments = vec!["kerbo", "fmt"];
    arguments.extend_from_slice(flags);
    arguments.push(path);
    let cli = try_parse(Lang::Spanish, arguments).expect("los argumentos son válidos");
    let (mut report, mut log) = (Vec::new(), Vec::new());
    let status = run(
        &cli.command,
        LangPreference {
            flag: None,
            locale: Lang::Spanish,
        },
        tree.path(),
        &mut Terminal {
            input: &mut &b""[..],
            report: &mut report,
            log: &mut log,
        },
    );
    Outcome {
        status,
        report: String::from_utf8_lossy(&report).into_owned(),
    }
}

fn read(tree: &TempTree, name: &str) -> String {
    fs::read_to_string(tree.path().join(name)).expect("se lee")
}

#[test]
fn check_lists_what_is_not_formatted_and_writes_nothing() {
    let tree = TempTree::new("fmt-check");
    tree.write("a.go", UNFORMATTED);
    tree.write("b.go", FORMATTED);
    let outcome = fmt(&tree, &["--check"]);
    assert_eq!(outcome.status, EXIT_FINDINGS);
    assert!(outcome.report.contains("a.go"), "{}", outcome.report);
    assert!(!outcome.report.contains("b.go"), "{}", outcome.report);
    assert_eq!(read(&tree, "a.go"), UNFORMATTED);
}

#[test]
fn formatting_writes_the_files_and_then_check_is_clean() {
    let tree = TempTree::new("fmt-write");
    tree.write("a.go", UNFORMATTED);
    assert_eq!(fmt(&tree, &[]).status, EXIT_CLEAN);
    assert_eq!(read(&tree, "a.go"), FORMATTED);
    assert_eq!(fmt(&tree, &["--check"]).status, EXIT_CLEAN);
}

#[test]
fn a_file_that_does_not_parse_is_left_alone_and_fails() {
    let tree = TempTree::new("fmt-broken");
    tree.write("broken.go", "package main\nfunc {\n");
    assert_eq!(fmt(&tree, &[]).status, EXIT_FAILURE);
    assert_eq!(read(&tree, "broken.go"), "package main\nfunc {\n");
}
