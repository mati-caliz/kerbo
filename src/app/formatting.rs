use std::fs;
use std::io::{self, Write};
use std::path::Path;

use rayon::prelude::*;

use super::fixing::write_atomically;
use super::{EXIT_CLEAN, EXIT_FAILURE, EXIT_FINDINGS, Terminal, write_error, write_failures};
use crate::cli::FmtArgs;
use crate::config::Config;
use crate::formatter::format_go;
use crate::i18n::{FormatNotice, Lang, Notice, Problem};
use crate::language::Language;
use crate::rules::FileIdentity;
use crate::scan::{ScanFailure, identities};

#[derive(Debug)]
enum Formatted {
    Unchanged,
    Changed,
    Unparsable,
}

pub fn run(
    arguments: &FmtArgs,
    directory: &Path,
    lang: Lang,
    terminal: &mut Terminal<'_>,
) -> Result<u8, Problem> {
    let config = Config::load(arguments.config.as_deref(), directory)?;
    let (files, mut failures) = identities(&arguments.paths, &config);
    let go_files: Vec<FileIdentity> = files
        .into_iter()
        .filter(|file| Language::from_path(&file.absolute) == Some(Language::Go))
        .collect();
    let write = !arguments.check;
    let results: Vec<(&FileIdentity, Result<Formatted, ScanFailure>)> = go_files
        .par_iter()
        .map(|file| (file, format_file(file, write)))
        .collect();
    let mut changed = Vec::new();
    let mut unparsable = Vec::new();
    for (file, result) in results {
        match result {
            Ok(Formatted::Changed) => changed.push(file.display.clone()),
            Ok(Formatted::Unparsable) => unparsable.push(file.display.clone()),
            Ok(Formatted::Unchanged) => {}
            Err(failure) => failures.push(failure),
        }
    }
    if arguments.check {
        for path in &changed {
            writeln!(terminal.report, "{path}").map_err(|error| write_error(&error))?;
        }
    }
    let total = go_files.len();
    let summary = if arguments.check {
        FormatNotice::Unformatted {
            files: changed.len(),
            total,
        }
    } else {
        FormatNotice::Written {
            files: changed.len(),
            total,
        }
    };
    report_notices(lang, terminal, &unparsable, summary)
        .and_then(|()| write_failures(&failures, lang, terminal.log))
        .map_err(|error| write_error(&error))?;
    Ok(if !failures.is_empty() || !unparsable.is_empty() {
        EXIT_FAILURE
    } else if arguments.check && !changed.is_empty() {
        EXIT_FINDINGS
    } else {
        EXIT_CLEAN
    })
}

fn report_notices(
    lang: Lang,
    terminal: &mut Terminal<'_>,
    unparsable: &[String],
    summary: FormatNotice,
) -> io::Result<()> {
    for path in unparsable {
        let notice = Notice::Format(FormatNotice::Unparsable { path: path.clone() });
        writeln!(terminal.log, "kerbo: {}", lang.notice(&notice))?;
    }
    writeln!(terminal.log, "kerbo: {}", lang.notice(&Notice::Format(summary)))
}

fn format_file(file: &FileIdentity, write: bool) -> Result<Formatted, ScanFailure> {
    let failure = |error: &io::Error| ScanFailure {
        problem: Problem::Unreadable {
            path: file.display.clone(),
            detail: error.to_string(),
        },
    };
    let original = fs::read_to_string(&file.absolute).map_err(|error| failure(&error))?;
    let Ok(formatted) = format_go(&original) else {
        return Ok(Formatted::Unparsable);
    };
    if formatted == original {
        return Ok(Formatted::Unchanged);
    }
    if write {
        write_atomically(&file.absolute, &formatted).map_err(|error| failure(&error))?;
    }
    Ok(Formatted::Changed)
}
