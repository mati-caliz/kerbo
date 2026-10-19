use std::path::{Path, PathBuf};

use clap::Parser;

use super::{EXIT_CLEAN, EXIT_FAILURE, EXIT_FINDINGS, Terminal, run};
use crate::cli::{Cli, Command};
use crate::test_support::TempTree;

const LONG_SIGNATURE: &str = "package sample\n\nfunc Many(a, b, c, d, e int) {}\n";

struct Outcome {
    status: u8,
    report: String,
    log: String,
}

fn execute(arguments: &[&str], directory: &Path) -> Outcome {
    let cli = Cli::try_parse_from(arguments).expect("los argumentos son válidos");
    let (mut report, mut log) = (Vec::new(), Vec::new());
    let status = run(
        &cli.command,
        directory,
        &mut Terminal {
            report: &mut report,
            log: &mut log,
        },
    );
    Outcome {
        status,
        report: String::from_utf8_lossy(&report).into_owned(),
        log: String::from_utf8_lossy(&log).into_owned(),
    }
}

fn path_of(tree: &TempTree) -> &str {
    tree.path().to_str().expect("la ruta temporal es UTF-8")
}

#[test]
fn parses_check_with_defaults_and_flags() {
    let cli = Cli::try_parse_from(["kerbo", "check"]).expect("es válido");
    let Command::Check(arguments) = cli.command else {
        panic!("se esperaba check");
    };
    assert_eq!(arguments.scan.paths, [PathBuf::from(".")]);
    assert!(arguments.diff.is_none() && !arguments.no_baseline && arguments.scan.config.is_none());
    let tuned = Cli::try_parse_from([
        "kerbo",
        "check",
        "--max",
        "parameter-count=0",
        "--diff",
        "main",
        "--no-baseline",
    ])
    .expect("es válido");
    let Command::Check(tuned) = tuned.command else {
        panic!("se esperaba check");
    };
    assert_eq!(
        (
            tuned.scan.overrides.len(),
            tuned.diff.as_deref(),
            tuned.no_baseline
        ),
        (1, Some("main"), true)
    );
    assert!(Cli::try_parse_from(["kerbo", "check", "--max", "nope=1"]).is_err());
    assert!(matches!(
        Cli::try_parse_from(["kerbo", "baseline"]).map(|parsed| parsed.command),
        Ok(Command::Baseline(_))
    ));
}

#[test]
fn reports_findings_and_summary() {
    let tree = TempTree::new("app-findings");
    tree.write("many.go", LONG_SIGNATURE);
    let outcome = execute(&["kerbo", "check", path_of(&tree)], tree.path());
    assert_eq!(outcome.status, EXIT_FINDINGS);
    assert!(outcome.report.contains("parameter-count"));
    assert!(outcome.log.contains("1 archivos, 1 problemas"));
    let sarif = execute(
        &["kerbo", "check", "--format", "sarif", path_of(&tree)],
        tree.path(),
    );
    assert!(sarif.report.contains("\"version\":\"2.1.0\""));
}

#[test]
fn baseline_hides_known_problems_until_they_change() {
    let tree = TempTree::new("app-baseline");
    tree.write("many.go", LONG_SIGNATURE);
    let saved = execute(&["kerbo", "baseline", path_of(&tree)], tree.path());
    assert_eq!(saved.status, EXIT_CLEAN);
    assert!(saved.log.contains("1 problemas guardados"));
    let after = execute(&["kerbo", "check", path_of(&tree)], tree.path());
    assert_eq!(after.status, EXIT_CLEAN);
    assert!(after.log.contains("0 problemas (y 1 en la línea de base)"));
    let everything = execute(&["kerbo", "check", "--no-baseline", path_of(&tree)], tree.path());
    assert_eq!(everything.status, EXIT_FINDINGS);
    tree.write(
        "many.go",
        "package sample\n\nfunc Many(a, b, c, d, e, f int) {}\n",
    );
    assert_eq!(
        execute(&["kerbo", "check", path_of(&tree)], tree.path()).status,
        EXIT_FINDINGS,
        "empeoró"
    );
}

#[test]
fn counts_configuration_deviations_and_fails_on_bad_setup() {
    let tree = TempTree::new("app-config");
    tree.write("many.go", LONG_SIGNATURE);
    tree.write(
        "kerbo.toml",
        "[[limits]]\nrule = \"parameter-count\"\nmax = 5\nreason = \"acordado\"\n",
    );
    let tuned = execute(&["kerbo", "check", path_of(&tree)], tree.path());
    assert_eq!(tuned.status, EXIT_CLEAN);
    assert!(tuned.log.contains("1 desvíos del preset"));
    tree.write("kerbo.toml", "preset = \"nope\"\n");
    let broken = execute(&["kerbo", "check", path_of(&tree)], tree.path());
    assert_eq!(broken.status, EXIT_FAILURE);
    assert!(broken.log.contains("kerbo.toml"));
    assert_eq!(
        execute(&["kerbo", "baseline", path_of(&tree)], tree.path()).status,
        EXIT_FAILURE
    );
}

#[test]
fn fails_on_missing_paths_and_outside_git_with_diff() {
    let tree = TempTree::new("app-failures");
    let missing = tree.path().join("missing");
    let missing = missing.to_str().expect("UTF-8");
    let outcome = execute(&["kerbo", "check", missing], tree.path());
    assert_eq!(outcome.status, EXIT_FAILURE);
    assert_eq!(
        execute(&["kerbo", "baseline", missing], tree.path()).status,
        EXIT_FAILURE
    );
    let no_git = execute(&["kerbo", "check", "--diff", "main", path_of(&tree)], tree.path());
    assert_eq!(no_git.status, EXIT_FAILURE);
    assert!(no_git.log.contains("git"));
}
