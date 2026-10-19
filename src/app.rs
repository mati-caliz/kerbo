use std::io::{self, Write};
use std::path::Path;
use std::time::Instant;

use crate::baseline::{BASELINE_FILE, Baseline};
use crate::cli::{CheckArgs, Command, ScanArgs};
use crate::config::Config;
use crate::diff::ChangedLines;
use crate::report::render;
use crate::rules::Diagnostic;
use crate::scan::{ScanFailure, ScanOutcome, ScanRequest, scan};

pub const EXIT_CLEAN: u8 = 0;
pub const EXIT_FINDINGS: u8 = 1;
pub const EXIT_FAILURE: u8 = 2;

pub struct Terminal<'output> {
    pub report: &'output mut dyn Write,
    pub log: &'output mut dyn Write,
}

struct Summary {
    files: usize,
    problems: usize,
    known: usize,
    deviations: usize,
    started: Instant,
}

pub fn run(command: &Command, directory: &Path, terminal: &mut Terminal<'_>) -> u8 {
    let result = match command {
        Command::Check(arguments) => check(arguments, directory, terminal),
        Command::Baseline(arguments) => baseline(arguments, directory, terminal),
    };
    result.unwrap_or_else(|error| {
        let _written = writeln!(terminal.log, "kerbo: {error}");
        EXIT_FAILURE
    })
}

fn check(arguments: &CheckArgs, directory: &Path, terminal: &mut Terminal<'_>) -> Result<u8, String> {
    let started = Instant::now();
    let config =
        Config::load(arguments.scan.config.as_deref(), directory).map_err(|error| error.to_string())?;
    let outcome = scan_with(&arguments.scan, &config);
    let mut diagnostics = outcome.diagnostics;
    if let Some(reference) = &arguments.diff {
        let changes = ChangedLines::since(reference, directory)?;
        diagnostics.retain(|diagnostic| changes.touches(diagnostic));
    }
    let mut known = 0;
    if !arguments.no_baseline
        && let Some(baseline) = Baseline::load(&config.root().join(BASELINE_FILE))?
    {
        let filtered = baseline.filter(diagnostics);
        known = filtered.known;
        diagnostics = filtered.diagnostics;
    }
    render(&diagnostics, arguments.format, terminal.report)
        .and_then(|()| terminal.report.flush())
        .map_err(|error| write_error(&error))?;
    let summary = Summary {
        files: outcome.files,
        problems: diagnostics.len(),
        known,
        deviations: config.deviations(),
        started,
    };
    write_failures(&outcome.failures, terminal.log)
        .and_then(|()| write_summary(&summary, terminal.log))
        .map_err(|error| write_error(&error))?;
    Ok(exit_status(&outcome.failures, &diagnostics))
}

fn baseline(arguments: &ScanArgs, directory: &Path, terminal: &mut Terminal<'_>) -> Result<u8, String> {
    let config = Config::load(arguments.config.as_deref(), directory).map_err(|error| error.to_string())?;
    let outcome = scan_with(arguments, &config);
    let path = config.root().join(BASELINE_FILE);
    let saved = Baseline::save(&outcome.diagnostics, &path)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    write_failures(&outcome.failures, terminal.log)
        .and_then(|()| {
            writeln!(
                terminal.log,
                "kerbo: {saved} problemas guardados en {}",
                path.display()
            )
        })
        .map_err(|error| write_error(&error))?;
    Ok(if outcome.failures.is_empty() {
        EXIT_CLEAN
    } else {
        EXIT_FAILURE
    })
}

fn scan_with(arguments: &ScanArgs, config: &Config) -> ScanOutcome {
    scan(&ScanRequest {
        roots: &arguments.paths,
        stacks: &arguments.languages,
        overrides: &arguments.overrides,
        config,
    })
}

fn write_failures(failures: &[ScanFailure], log: &mut dyn Write) -> io::Result<()> {
    for failure in failures {
        writeln!(log, "kerbo: {} {}", failure.path, failure.reason)?;
    }
    Ok(())
}

fn write_summary(summary: &Summary, log: &mut dyn Write) -> io::Result<()> {
    write!(
        log,
        "kerbo: {} archivos, {} problemas",
        summary.files, summary.problems
    )?;
    if summary.known > 0 {
        write!(log, " (y {} en la línea de base)", summary.known)?;
    }
    write!(log, ", {} ms", summary.started.elapsed().as_millis())?;
    if summary.deviations > 0 {
        write!(log, " · {} desvíos del preset en kerbo.toml", summary.deviations)?;
    }
    writeln!(log)
}

fn write_error(error: &io::Error) -> String {
    format!("no se pudo escribir el reporte: {error}")
}

const fn exit_status(failures: &[ScanFailure], diagnostics: &[Diagnostic]) -> u8 {
    if !failures.is_empty() {
        EXIT_FAILURE
    } else if diagnostics.is_empty() {
        EXIT_CLEAN
    } else {
        EXIT_FINDINGS
    }
}

#[cfg(test)]
mod tests;
