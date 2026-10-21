use std::io::{self, Read, Write};
use std::path::Path;
use std::time::Instant;

use crate::baseline::{BASELINE_FILE, Baseline};
use crate::cache::AnalysisCache;
use crate::cli::{CheckArgs, Command, DaemonAction, ScanArgs};
use crate::config::Config;
use crate::daemon;
use crate::diff::ChangedLines;
use crate::guard::{GuardRequest, add_guarded, inspect};
use crate::i18n::{Lang, LangPreference, Notice, Problem, Summary};
use crate::osv::{AuditRequest, Curl, Ecosystem, OsvMode, audit, download};
use crate::report::{Format, render, render_sarif};
use crate::rules::{Diagnostic, RuleId};
use crate::scan::{ScanFailure, ScanOutcome, ScanRequest, scan};
use crate::suppressions::Suppressed;

pub const EXIT_CLEAN: u8 = 0;
pub const EXIT_FINDINGS: u8 = 1;
pub const EXIT_FAILURE: u8 = 2;

pub struct Terminal<'output> {
    pub input: &'output mut dyn Read,
    pub report: &'output mut dyn Write,
    pub log: &'output mut dyn Write,
}

pub fn run(
    command: &Command,
    preference: LangPreference,
    directory: &Path,
    terminal: &mut Terminal<'_>,
) -> u8 {
    let lang = preference.resolve(configured_lang(command.config(), directory));
    let (result, failure_status) = match command {
        Command::Check(arguments) => (check(arguments, directory, lang, terminal), EXIT_FAILURE),
        Command::Baseline(arguments) => (baseline(arguments, directory, lang, terminal), EXIT_FAILURE),
        Command::Explain { rule } => (explain_rule(*rule, lang, terminal), EXIT_FAILURE),
        Command::Fix(arguments) => (fixing::run(arguments, directory, lang, terminal), EXIT_FAILURE),
        Command::Hook(arguments) => (
            hook::run(arguments, directory, lang, terminal),
            hook::NON_BLOCKING_ERROR,
        ),
        Command::Mcp => (mcp::serve(directory, lang, terminal), EXIT_FAILURE),
        Command::OsvDownload { ecosystems } => (download_osv(ecosystems, lang, terminal), EXIT_FAILURE),
        Command::Lsp => (lsp::serve(lang, terminal), EXIT_FAILURE),
        Command::Daemon { action } => (daemon_action(*action, lang, terminal), EXIT_FAILURE),
    };
    result.unwrap_or_else(|problem| {
        let _written = writeln!(terminal.log, "kerbo: {}", lang.problem(&problem));
        failure_status
    })
}

pub fn configured_lang(explicit: Option<&Path>, directory: &Path) -> Option<Lang> {
    Config::load(explicit, directory)
        .ok()
        .and_then(|config| config.lang())
}

pub struct Findings {
    pub files: usize,
    pub diagnostics: Vec<Diagnostic>,
    pub known: usize,
    pub deviations: usize,
    pub suppressed: Vec<Suppressed>,
    pub failures: Vec<ScanFailure>,
}

pub struct FindingsRequest<'request> {
    pub scan: &'request ScanArgs,
    pub diff: Option<&'request str>,
    pub use_baseline: bool,
    pub osv: Option<OsvMode>,
}

pub fn findings(request: &FindingsRequest<'_>, directory: &Path) -> Result<Findings, Problem> {
    let config = Config::load(request.scan.config.as_deref(), directory)?;
    let outcome = scan_with(request.scan, &config);
    let mut diagnostics = outcome.diagnostics;
    let mut suppressed = outcome.suppressed;
    if let Some(mode) = request.osv {
        diagnostics.extend(audit(&AuditRequest {
            roots: &request.scan.paths,
            config: &config,
            mode,
            transport: &Curl::default(),
            directory: None,
        })?);
    }
    if let Some(reference) = request.diff {
        let changes = ChangedLines::since(reference, directory)?;
        diagnostics.retain(|diagnostic| changes.touches(diagnostic));
        suppressed.retain(|item| changes.touches(&item.diagnostic));
        let project_root = config
            .root()
            .canonicalize()
            .unwrap_or_else(|_| config.root().to_path_buf());
        let guarded = inspect(&GuardRequest {
            changes: &changes,
            config: &config,
            project_root: &project_root,
            stacks: &request.scan.languages,
        });
        add_guarded(&mut diagnostics, guarded);
    }
    let mut known = 0;
    if request.use_baseline
        && let Some(baseline) = Baseline::load(&config.root().join(BASELINE_FILE))?
    {
        let filtered = baseline.filter(diagnostics);
        known = filtered.known;
        diagnostics = filtered.diagnostics;
    }
    Ok(Findings {
        files: outcome.files,
        diagnostics,
        known,
        deviations: config.deviations(),
        suppressed,
        failures: outcome.failures,
    })
}

fn check(
    arguments: &CheckArgs,
    directory: &Path,
    lang: Lang,
    terminal: &mut Terminal<'_>,
) -> Result<u8, Problem> {
    let started = Instant::now();
    let request = FindingsRequest {
        scan: &arguments.scan,
        diff: arguments.diff.as_deref(),
        use_baseline: !arguments.no_baseline,
        osv: arguments.osv,
    };
    let found = findings(&request, directory)?;
    let rendered = if arguments.format == Format::Sarif {
        render_sarif(&found.diagnostics, &found.suppressed, lang, terminal.report)
    } else {
        render(&found.diagnostics, arguments.format, lang, terminal.report)
    };
    rendered
        .and_then(|()| terminal.report.flush())
        .map_err(|error| write_error(&error))?;
    let summary = Notice::Summary(Summary {
        files: found.files,
        problems: found.diagnostics.len(),
        known: found.known,
        milliseconds: started.elapsed().as_millis(),
        deviations: found.deviations,
        suppressed: found.suppressed.len(),
    });
    write_failures(&found.failures, lang, terminal.log)
        .and_then(|()| write_suppressed(&found.suppressed, lang, terminal.log))
        .and_then(|()| writeln!(terminal.log, "kerbo: {}", lang.notice(&summary)))
        .map_err(|error| write_error(&error))?;
    Ok(exit_status(&found.failures, &found.diagnostics))
}

fn baseline(
    arguments: &ScanArgs,
    directory: &Path,
    lang: Lang,
    terminal: &mut Terminal<'_>,
) -> Result<u8, Problem> {
    let config = Config::load(arguments.config.as_deref(), directory)?;
    let outcome = scan_with(arguments, &config);
    let path = config.root().join(BASELINE_FILE);
    let saved = Baseline::save(&outcome.diagnostics, &path).map_err(|error| Problem::Unreadable {
        path: path.display().to_string(),
        detail: error.to_string(),
    })?;
    let notice = Notice::BaselineSaved {
        count: saved,
        path: path.display().to_string(),
    };
    write_failures(&outcome.failures, lang, terminal.log)
        .and_then(|()| writeln!(terminal.log, "kerbo: {}", lang.notice(&notice)))
        .map_err(|error| write_error(&error))?;
    Ok(if outcome.failures.is_empty() {
        EXIT_CLEAN
    } else {
        EXIT_FAILURE
    })
}

fn download_osv(ecosystems: &[Ecosystem], lang: Lang, terminal: &mut Terminal<'_>) -> Result<u8, Problem> {
    for notice in download(ecosystems, &Curl::default(), None)? {
        writeln!(terminal.log, "kerbo: {}", lang.notice(&notice)).map_err(|error| write_error(&error))?;
    }
    Ok(EXIT_CLEAN)
}

fn daemon_action(action: DaemonAction, lang: Lang, terminal: &mut Terminal<'_>) -> Result<u8, Problem> {
    let directory = daemon::endpoint_directory().ok_or(Problem::DaemonWithoutDirectory)?;
    let notice = match action {
        DaemonAction::Start => daemon::start(&directory)?,
        DaemonAction::Stop => daemon::stop(&directory),
        DaemonAction::Status => daemon::status(&directory),
        DaemonAction::Run => {
            daemon::serve(&directory, &daemon::default_lifetime(&directory))
                .map_err(|error| Problem::DaemonUnavailable(error.to_string()))?;
            return Ok(EXIT_CLEAN);
        }
    };
    writeln!(terminal.log, "kerbo: {}", lang.notice(&notice)).map_err(|error| write_error(&error))?;
    Ok(EXIT_CLEAN)
}

fn explain_rule(rule: RuleId, lang: Lang, terminal: &mut Terminal<'_>) -> Result<u8, Problem> {
    writeln!(terminal.report, "{}", lang.explanation_text(rule)).map_err(|error| write_error(&error))?;
    Ok(EXIT_CLEAN)
}

fn scan_with(arguments: &ScanArgs, config: &Config) -> ScanOutcome {
    let cache = if arguments.no_cache {
        None
    } else {
        AnalysisCache::shared(config.cache_max_megabytes())
    };
    scan(&ScanRequest {
        roots: &arguments.paths,
        stacks: &arguments.languages,
        overrides: &arguments.overrides,
        config,
        cache: cache.as_deref(),
    })
}

fn write_failures(failures: &[ScanFailure], lang: Lang, log: &mut dyn Write) -> io::Result<()> {
    for failure in failures {
        writeln!(log, "kerbo: {}", lang.problem(&failure.problem))?;
    }
    Ok(())
}

fn write_suppressed(suppressed: &[Suppressed], lang: Lang, log: &mut dyn Write) -> io::Result<()> {
    for item in suppressed {
        let notice = Notice::Suppressed {
            path: item.diagnostic.path.clone(),
            line: item.diagnostic.line,
            rule: item.diagnostic.rule,
            reason: item.reason.clone(),
        };
        writeln!(log, "kerbo: {}", lang.notice(&notice))?;
    }
    Ok(())
}

fn write_error(error: &io::Error) -> Problem {
    Problem::WriteReport(error.to_string())
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

mod fixing;
mod hook;
mod lsp;
mod mcp;

#[cfg(test)]
mod tests;
