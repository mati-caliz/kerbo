use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::{EXIT_CLEAN, Terminal, write_error};
use crate::baseline::{BASELINE_FILE, Baseline};
use crate::cli::HookArgs;
use crate::config::Config;
use crate::diff::ChangedLines;
use crate::guard::{GuardRequest, add_guarded, inspect};
use crate::i18n::{Lang, Notice, Problem};
use crate::language::Language;
use crate::report::{Format, render};
use crate::rules::Diagnostic;
use crate::scan::{ScanRequest, scan};

pub const NON_BLOCKING_ERROR: u8 = 1;
const FEEDBACK_FOR_AGENT: u8 = 2;
const COMPARED_REVISION: &str = "HEAD";

#[derive(Debug, Deserialize)]
struct HookEvent {
    cwd: Option<PathBuf>,
    tool_input: Option<ToolInput>,
}

#[derive(Debug, Deserialize)]
struct ToolInput {
    file_path: Option<PathBuf>,
}

pub fn run(
    arguments: &HookArgs,
    directory: &Path,
    lang: Lang,
    terminal: &mut Terminal<'_>,
) -> Result<u8, Problem> {
    let mut text = String::new();
    terminal
        .input
        .read_to_string(&mut text)
        .map_err(|error| Problem::ReadEvent(error.to_string()))?;
    let event: HookEvent =
        serde_json::from_str(&text).map_err(|error| Problem::InvalidEvent(error.to_string()))?;
    let base = event.cwd.unwrap_or_else(|| directory.to_path_buf());
    let Some(file) = event
        .tool_input
        .and_then(|input| input.file_path)
        .map(|file| base.join(file))
    else {
        return Ok(EXIT_CLEAN);
    };
    if Language::from_path(&file).is_none() || !file.is_file() {
        return Ok(EXIT_CLEAN);
    }
    let diagnostics = review(&file, arguments)?;
    if diagnostics.is_empty() {
        return Ok(EXIT_CLEAN);
    }
    write_feedback(&diagnostics, lang, terminal.log).map_err(|error| write_error(&error))?;
    Ok(FEEDBACK_FOR_AGENT)
}

fn review(file: &Path, arguments: &HookArgs) -> Result<Vec<Diagnostic>, Problem> {
    let start = file.parent().unwrap_or(file);
    let config = Config::load(arguments.config.as_deref(), start)?;
    let roots = [file.to_path_buf()];
    let outcome = scan(&ScanRequest {
        roots: &roots,
        stacks: &[],
        overrides: &[],
        config: &config,
        cache: None,
    });
    let mut diagnostics = outcome.diagnostics;
    let absolute = file.canonicalize().unwrap_or_else(|_| file.to_path_buf());
    if let Ok(changes) = ChangedLines::since(COMPARED_REVISION, start) {
        diagnostics.retain(|diagnostic| changes.touches(diagnostic));
        let project_root = config
            .root()
            .canonicalize()
            .unwrap_or_else(|_| config.root().to_path_buf());
        let request = GuardRequest {
            changes: &changes,
            config: &config,
            project_root: &project_root,
            stacks: &[],
        };
        add_guarded(
            &mut diagnostics,
            inspect(&request)
                .into_iter()
                .filter(|diagnostic| diagnostic.absolute_path == absolute),
        );
    }
    match Baseline::load(&config.root().join(BASELINE_FILE))? {
        Some(baseline) => Ok(baseline.filter(diagnostics).diagnostics),
        None => Ok(diagnostics),
    }
}

fn write_feedback(diagnostics: &[Diagnostic], lang: Lang, log: &mut dyn Write) -> std::io::Result<()> {
    let header = Notice::HookHeader {
        count: diagnostics.len(),
    };
    writeln!(log, "kerbo: {}", lang.notice(&header))?;
    render(diagnostics, Format::Agent, lang, log)
}
