use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use rayon::prelude::*;

use super::{EXIT_CLEAN, EXIT_FAILURE, EXIT_FINDINGS, Terminal, scan_with, write_error};
use crate::cli::{FixArgs, ScanArgs};
use crate::config::Config;
use crate::diff::ChangedLines;
use crate::fix::{Fix, apply, compatible, edits_of, file_patch};
use crate::i18n::{FixNotice, Lang, Notice, Problem};
use crate::language::{Language, Stack};
use crate::report::{Format, render};
use crate::rules::{Diagnostic, RuleId};
use crate::scan::{ScanFailure, check_text};
use crate::syntax::syntax_errors;

const MAX_FIX_PASSES: usize = 10;
const TEMPORARY_SUFFIX: &str = ".kerbo-fix.tmp";
const HIDDEN_PREFIX: &str = ".";

#[derive(Debug)]
pub struct FixRequest<'request> {
    pub paths: Vec<PathBuf>,
    pub languages: &'request [Stack],
    pub rules: &'request [RuleId],
    pub config: Option<&'request Path>,
    pub write: bool,
    pub since: Option<&'request str>,
}

#[derive(Debug)]
pub struct FixedFile {
    pub path: String,
    pub applied: usize,
    pub left: usize,
    pub patch: String,
    pub introduced: Vec<Diagnostic>,
    pub written: bool,
}

#[derive(Debug, Default)]
pub struct FixOutcome {
    pub files: Vec<FixedFile>,
    pub failures: Vec<ScanFailure>,
}

struct Target<'target> {
    absolute: &'target Path,
    config: &'target Config,
    rules: &'target [RuleId],
    allowed: Option<&'target HashSet<String>>,
}

// Cómo se arregla un texto: qué lo analiza y qué arreglos se aceptan. Lo usan `kerbo fix` y el
// "arreglar todo" del LSP.
pub struct Fixer<'fixer> {
    pub language: Option<Language>,
    pub analyze: &'fixer dyn Fn(&str) -> Option<Vec<Diagnostic>>,
    pub accepts: &'fixer dyn Fn(&Diagnostic) -> bool,
}

pub struct Converged {
    pub text: String,
    pub applied: usize,
    pub after: Vec<Diagnostic>,
}

impl FixOutcome {
    pub fn withheld(&self) -> impl Iterator<Item = &FixedFile> {
        self.files.iter().filter(|file| !file.introduced.is_empty())
    }

    pub fn patch(&self) -> String {
        self.files.iter().map(|file| file.patch.as_str()).collect()
    }

    pub fn notices(&self, lang: Lang) -> Vec<String> {
        let accepted: Vec<&FixedFile> = self
            .files
            .iter()
            .filter(|file| file.introduced.is_empty())
            .collect();
        let count = accepted.iter().map(|file| file.applied).sum();
        let files = accepted.len();
        let mut notices = Vec::new();
        for file in self.withheld() {
            let withheld = Notice::Fix(FixNotice::Withheld {
                path: file.path.clone(),
                introduced: file.introduced.len(),
            });
            notices.push(lang.notice(&withheld));
            let mut rendered = Vec::new();
            if render(&file.introduced, Format::Text, lang, &mut rendered).is_ok() {
                notices.extend(String::from_utf8_lossy(&rendered).lines().map(str::to_owned));
            }
        }
        let left: usize = self.files.iter().map(|file| file.left).sum();
        if left > 0 {
            notices.push(lang.notice(&Notice::Fix(FixNotice::Left { count: left })));
        }
        let written = self.files.iter().any(|file| file.written);
        let summary = if written || self.files.is_empty() {
            FixNotice::Applied { count, files }
        } else {
            FixNotice::Previewed { count, files }
        };
        notices.push(lang.notice(&Notice::Fix(summary)));
        notices.extend(self.failures.iter().map(|failure| lang.problem(&failure.problem)));
        notices
    }
}

pub fn run(
    arguments: &FixArgs,
    directory: &Path,
    lang: Lang,
    terminal: &mut Terminal<'_>,
) -> Result<u8, Problem> {
    let request = FixRequest {
        paths: arguments.paths.clone(),
        languages: &arguments.languages,
        rules: &arguments.rules,
        config: arguments.config.as_deref(),
        write: !arguments.dry_run,
        since: arguments.since.as_deref(),
    };
    let outcome = fix_files(&request, directory)?;
    if arguments.dry_run {
        write!(terminal.report, "{}", outcome.patch())
            .and_then(|()| terminal.report.flush())
            .map_err(|error| write_error(&error))?;
    }
    for notice in outcome.notices(lang) {
        writeln!(terminal.log, "kerbo: {notice}").map_err(|error| write_error(&error))?;
    }
    Ok(if !outcome.failures.is_empty() {
        EXIT_FAILURE
    } else if outcome.withheld().next().is_some() {
        EXIT_FINDINGS
    } else {
        EXIT_CLEAN
    })
}

pub fn fix_files(request: &FixRequest<'_>, directory: &Path) -> Result<FixOutcome, Problem> {
    let config = Config::load(request.config, directory)?;
    let scan = ScanArgs {
        paths: request.paths.clone(),
        languages: request.languages.to_vec(),
        overrides: Vec::new(),
        config: request.config.map(Path::to_path_buf),
        no_cache: false,
    };
    let outcome = scan_with(&scan, &config);
    let changes = request
        .since
        .map(|reference| ChangedLines::since(reference, directory))
        .transpose()?;
    let mut candidates: BTreeMap<PathBuf, (String, HashSet<String>)> = BTreeMap::new();
    for diagnostic in outcome.diagnostics {
        let touched = changes
            .as_ref()
            .is_none_or(|changes| changes.touches(&diagnostic));
        if touched && wanted_fix(&diagnostic, request.rules).is_some() {
            candidates
                .entry(diagnostic.absolute_path)
                .or_insert_with(|| (diagnostic.relative_path, HashSet::new()))
                .1
                .insert(diagnostic.fingerprint);
        }
    }
    let results: Vec<Result<Option<FixedFile>, ScanFailure>> = candidates
        .par_iter()
        .map(|(absolute, (relative, touched))| {
            let target = Target {
                absolute,
                config: &config,
                rules: request.rules,
                allowed: changes.as_ref().map(|_| touched),
            };
            fix_file(&target, relative, request.write)
        })
        .collect();
    let mut fixed = FixOutcome {
        failures: outcome.failures,
        ..FixOutcome::default()
    };
    for result in results {
        match result {
            Ok(file) => fixed.files.extend(file),
            Err(failure) => fixed.failures.push(failure),
        }
    }
    Ok(fixed)
}

fn fix_file(target: &Target<'_>, relative: &str, write: bool) -> Result<Option<FixedFile>, ScanFailure> {
    let failure = |error: &io::Error| ScanFailure {
        problem: Problem::Unreadable {
            path: relative.to_owned(),
            detail: error.to_string(),
        },
    };
    let original = fs::read_to_string(target.absolute).map_err(|error| failure(&error))?;
    let Some(before) = check_text(target.absolute, original.as_bytes(), target.config) else {
        return Ok(None);
    };
    let analyze = |text: &str| check_text(target.absolute, text.as_bytes(), target.config);
    let accepts = |diagnostic: &Diagnostic| {
        wanted_fix(diagnostic, target.rules).is_some()
            && target
                .allowed
                .is_none_or(|allowed| allowed.contains(&diagnostic.fingerprint))
    };
    let fixer = Fixer {
        language: Language::from_path(target.absolute),
        analyze: &analyze,
        accepts: &accepts,
    };
    let Some(converged) = converge(&fixer, &original, before.clone()) else {
        return Ok(None);
    };
    let known: HashSet<&str> = before
        .iter()
        .map(|diagnostic| diagnostic.fingerprint.as_str())
        .collect();
    let left = converged
        .after
        .iter()
        .filter(|diagnostic| accepts(diagnostic))
        .count();
    let introduced: Vec<Diagnostic> = converged
        .after
        .into_iter()
        .filter(|diagnostic| !known.contains(diagnostic.fingerprint.as_str()))
        .collect();
    let written = write && introduced.is_empty();
    if written {
        write_atomically(target.absolute, &converged.text).map_err(|error| failure(&error))?;
    }
    Ok(Some(FixedFile {
        path: relative.to_owned(),
        applied: converged.applied,
        left,
        patch: file_patch(relative, &original, &converged.text).unwrap_or_default(),
        introduced,
        written,
    }))
}

// Aplica los arreglos compatibles, vuelve a analizar y repite: lo que destapa una vuelta (una clase
// que pasa a `final` deja de más el `final` de sus métodos) entra en la siguiente. Una vuelta que
// deja más errores de sintaxis de los que había no se acepta: tree-sitter se recupera y el análisis
// no lo notaría.
pub fn converge(fixer: &Fixer<'_>, original: &str, before: Vec<Diagnostic>) -> Option<Converged> {
    let errors_before = fixer
        .language
        .and_then(|language| syntax_errors(language, original.as_bytes()));
    let mut converged = Converged {
        text: original.to_owned(),
        applied: 0,
        after: before,
    };
    for _ in 0..MAX_FIX_PASSES {
        let chosen = compatible(
            converged
                .after
                .iter()
                .filter(|diagnostic| (fixer.accepts)(diagnostic))
                .filter_map(|diagnostic| diagnostic.fix.as_ref()),
        );
        if chosen.is_empty() {
            break;
        }
        let Some(text) = apply(&edits_of(&chosen), &converged.text) else {
            break;
        };
        let breaks_syntax = fixer
            .language
            .zip(errors_before)
            .is_some_and(|(language, errors)| {
                syntax_errors(language, text.as_bytes()).is_some_and(|after| after > errors)
            });
        if breaks_syntax {
            break;
        }
        let Some(after) = (fixer.analyze)(&text) else {
            break;
        };
        converged.applied = converged.applied.saturating_add(chosen.len());
        converged.after = after;
        converged.text = text;
    }
    (converged.applied > 0).then_some(converged)
}

pub fn wanted_fix<'diagnostic>(
    diagnostic: &'diagnostic Diagnostic,
    rules: &[RuleId],
) -> Option<&'diagnostic Fix> {
    let wanted = rules.is_empty() || rules.contains(&diagnostic.rule);
    diagnostic.fix.as_ref().filter(|_| wanted)
}

fn write_atomically(path: &Path, contents: &str) -> io::Result<()> {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_default();
    let temporary = path.with_file_name(format!("{HIDDEN_PREFIX}{name}{TEMPORARY_SUFFIX}"));
    let written = fs::write(&temporary, contents)
        .and_then(|()| fs::metadata(path))
        .and_then(|metadata| fs::set_permissions(&temporary, metadata.permissions()))
        .and_then(|()| fs::rename(&temporary, path));
    if written.is_err() {
        let _removed = fs::remove_file(&temporary);
    }
    written
}

#[cfg(test)]
mod tests;
