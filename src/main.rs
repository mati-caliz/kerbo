mod analysis;
mod language;
mod lines;
mod metrics;
mod profile;
mod report;
mod rules;
mod scan;
mod syntax;
#[cfg(test)]
mod test_support;

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use clap::{Args, Parser, Subcommand};

use crate::language::Stack;
use crate::report::{Format, render};
use crate::rules::LimitOverride;
use crate::scan::{ScanOutcome, scan};

const EXIT_CLEAN: u8 = 0;
const EXIT_FINDINGS: u8 = 1;
const EXIT_FAILURE: u8 = 2;

#[derive(Debug, Parser)]
#[command(name = "kerbo", version, about = "Analizador de calidad multi-lenguaje")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    #[command(about = "Analiza los archivos y lista los problemas")]
    Check(CheckArgs),
}

#[derive(Debug, Args)]
struct CheckArgs {
    #[arg(default_value = ".", help = "Archivos o carpetas a analizar")]
    paths: Vec<PathBuf>,
    #[arg(long, value_enum, default_value_t = Format::Text, help = "Formato de salida")]
    format: Format,
    #[arg(
        long = "language",
        value_enum,
        help = "Analizar sólo estas pilas (se puede repetir)"
    )]
    languages: Vec<Stack>,
    #[arg(
        long = "max",
        value_name = "REGLA=VALOR",
        help = "Cambia el máximo de una regla en todos los lenguajes, también en los tests (se puede repetir)"
    )]
    overrides: Vec<LimitOverride>,
}

fn main() -> ExitCode {
    let Command::Check(arguments) = Cli::parse().command;
    ExitCode::from(run(
        &arguments,
        &mut io::stdout().lock(),
        &mut io::stderr().lock(),
    ))
}

fn run(arguments: &CheckArgs, report: &mut impl Write, log: &mut impl Write) -> u8 {
    let started = Instant::now();
    let outcome = scan(&arguments.paths, &arguments.languages, &arguments.overrides);
    let written = render(&outcome.diagnostics, arguments.format, report)
        .and_then(|()| report.flush())
        .and_then(|()| write_summary(&outcome, started, log));
    match written {
        Ok(()) => exit_status(&outcome),
        Err(_) => EXIT_FAILURE,
    }
}

fn write_summary(outcome: &ScanOutcome, started: Instant, log: &mut impl Write) -> io::Result<()> {
    for failure in &outcome.failures {
        writeln!(log, "kerbo: {} {}", failure.path, failure.reason)?;
    }
    writeln!(
        log,
        "kerbo: {} archivos, {} problemas, {} ms",
        outcome.files,
        outcome.diagnostics.len(),
        started.elapsed().as_millis()
    )
}

const fn exit_status(outcome: &ScanOutcome) -> u8 {
    if !outcome.failures.is_empty() {
        EXIT_FAILURE
    } else if outcome.diagnostics.is_empty() {
        EXIT_CLEAN
    } else {
        EXIT_FINDINGS
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use std::path::PathBuf;

    use super::{CheckArgs, Cli, Command, EXIT_CLEAN, EXIT_FAILURE, EXIT_FINDINGS, run};
    use crate::report::Format;
    use crate::test_support::TempTree;

    fn arguments(path: PathBuf, format: Format) -> CheckArgs {
        CheckArgs {
            paths: vec![path],
            format,
            languages: Vec::new(),
            overrides: Vec::new(),
        }
    }

    const LONG_SIGNATURE: &str = "package sample\n\nfunc Many(a, b, c, d, e int) {}\n";

    #[test]
    fn parses_check_with_default_path_and_format() {
        let cli = Cli::try_parse_from(["kerbo", "check"]).expect("los argumentos son válidos");
        let Command::Check(arguments) = cli.command;
        assert_eq!(arguments.paths, [PathBuf::from(".")]);
        assert_eq!(arguments.format, Format::Text);
        assert_eq!(arguments.languages, []);
        let tuned = Cli::try_parse_from(["kerbo", "check", "--max", "parameter-count=0"]).expect("es válido");
        let Command::Check(tuned) = tuned.command;
        assert_eq!(tuned.overrides.len(), 1);
        assert!(Cli::try_parse_from(["kerbo", "check", "--max", "nope=1"]).is_err());
    }

    #[test]
    fn exits_with_findings_and_writes_report_and_summary() {
        let tree = TempTree::new("run-findings");
        tree.write("many.go", LONG_SIGNATURE);
        let (mut report, mut log) = (Vec::new(), Vec::new());
        let status = run(
            &arguments(tree.path().to_path_buf(), Format::Text),
            &mut report,
            &mut log,
        );
        assert_eq!(status, EXIT_FINDINGS);
        assert!(String::from_utf8_lossy(&report).contains("parameter-count"));
        assert!(String::from_utf8_lossy(&log).contains("1 archivos, 1 problemas"));
    }

    #[test]
    fn exits_clean_when_nothing_exceeds_the_limits() {
        let tree = TempTree::new("run-clean");
        tree.write("few.go", "package sample\n\nfunc Few(a int) {}\n");
        let status = run(
            &arguments(tree.path().to_path_buf(), Format::Json),
            &mut Vec::new(),
            &mut Vec::new(),
        );
        assert_eq!(status, EXIT_CLEAN);
    }

    #[test]
    fn exits_with_failure_when_a_path_does_not_exist() {
        let tree = TempTree::new("run-missing");
        let (mut report, mut log) = (Vec::new(), Vec::new());
        let status = run(
            &arguments(tree.path().join("missing"), Format::Text),
            &mut report,
            &mut log,
        );
        assert_eq!(status, EXIT_FAILURE);
        assert!(String::from_utf8_lossy(&log).lines().count() >= 2);
    }
}
