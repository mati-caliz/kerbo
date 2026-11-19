use std::fs;
use std::io;
use std::path::Path;

use rayon::prelude::*;

use super::fixing::write_atomically;
use super::{EXIT_CLEAN, EXIT_FAILURE, EXIT_FINDINGS, Terminal, write_error, write_failures};
use crate::cli::FmtArgs;
use crate::config::Config;
use crate::formatter::{Unparsable, format_go, format_python};
use crate::i18n::{FormatNotice, Lang, Notice, Problem};
use crate::language::Language;
use crate::rules::FileIdentity;
use crate::scan::{ScanFailure, identities};

type Formatter = fn(&str) -> Result<String, Unparsable>;

const PYTHON_EXTENSION: &str = "py";

#[derive(Debug)]
enum Formatted {
    Unchanged,
    Changed,
    Unparsable(Unparsable),
}

pub fn run(
    arguments: &FmtArgs,
    directory: &Path,
    lang: Lang,
    terminal: &mut Terminal<'_>,
) -> Result<u8, Problem> {
    let config = Config::load(arguments.config.as_deref(), directory)?;
    let (files, mut failures) = identities(&arguments.paths, &config);
    let formattable: Vec<(FileIdentity, Formatter)> = files
        .into_iter()
        .filter_map(|file| formatter_of(&file, arguments.python).map(|formatter| (file, formatter)))
        .collect();
    let write = !arguments.check;
    let results: Vec<(&FileIdentity, Result<Formatted, ScanFailure>)> = formattable
        .par_iter()
        .map(|(file, formatter)| (file, format_file(file, *formatter, write)))
        .collect();
    let mut changed = Vec::new();
    let mut unparsable = Vec::new();
    for (file, result) in results {
        match result {
            Ok(Formatted::Changed) => changed.push(file.display.clone()),
            Ok(Formatted::Unparsable(cause)) => unparsable.push((file.display.clone(), cause)),
            Ok(Formatted::Unchanged) => {}
            Err(failure) => failures.push(failure),
        }
    }
    if arguments.check {
        for path in &changed {
            writeln!(terminal.report, "{path}").map_err(|error| write_error(&error))?;
        }
    }
    let total = formattable.len();
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
    unparsable: &[(String, Unparsable)],
    summary: FormatNotice,
) -> io::Result<()> {
    for (path, cause) in unparsable {
        let notice = Notice::Format(FormatNotice::Unparsable {
            path: path.clone(),
            cause: cause.clone(),
        });
        writeln!(terminal.log, "kerbo: {}", lang.notice(&notice))?;
    }
    writeln!(terminal.log, "kerbo: {}", lang.notice(&Notice::Format(summary)))
}

// El formateador de Python está en construcción: sólo corre con el flag oculto `--python`, hasta
// que su salida sea idéntica a la de `ruff format` en el corpus.
fn formatter_of(file: &FileIdentity, with_python: bool) -> Option<Formatter> {
    let language = Language::from_path(&file.absolute)?;
    if language == Language::Go {
        return Some(format_go);
    }
    let is_python_source = language == Language::Python
        && file
            .absolute
            .extension()
            .is_some_and(|extension| extension == PYTHON_EXTENSION);
    (with_python && is_python_source).then_some(format_python)
}

fn format_file(file: &FileIdentity, formatter: Formatter, write: bool) -> Result<Formatted, ScanFailure> {
    let failure = |error: &io::Error| ScanFailure {
        problem: Problem::Unreadable {
            path: file.display.clone(),
            detail: error.to_string(),
        },
    };
    let original = fs::read_to_string(&file.absolute).map_err(|error| failure(&error))?;
    let output = match formatter(&original) {
        Ok(output) => output,
        Err(cause) => return Ok(Formatted::Unparsable(cause)),
    };
    if output == original {
        return Ok(Formatted::Unchanged);
    }
    if write {
        write_atomically(&file.absolute, &output).map_err(|error| failure(&error))?;
    }
    Ok(Formatted::Changed)
}

#[cfg(test)]
mod tests;
