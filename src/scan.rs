use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fs;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;
use rayon::prelude::*;
use tree_sitter::Parser;

use crate::analysis::analyze;
use crate::config::Config;
use crate::language::{Language, Stack};
use crate::limits::LimitOverride;
use crate::rules::{Diagnostic, FileIdentity, evaluate};

const PATH_SEPARATOR: &str = "/";

#[derive(Debug)]
pub struct ScanFailure {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Default)]
pub struct ScanOutcome {
    pub files: usize,
    pub diagnostics: Vec<Diagnostic>,
    pub failures: Vec<ScanFailure>,
}

#[derive(Debug)]
pub struct ScanRequest<'request> {
    pub roots: &'request [PathBuf],
    pub stacks: &'request [Stack],
    pub overrides: &'request [LimitOverride],
    pub config: &'request Config,
}

#[derive(Default)]
struct ParserPool {
    parsers: HashMap<Language, Parser>,
}

impl ParserPool {
    fn parser(&mut self, language: Language) -> Result<&mut Parser, String> {
        match self.parsers.entry(language) {
            Entry::Occupied(entry) => Ok(entry.into_mut()),
            Entry::Vacant(entry) => {
                let mut parser = Parser::new();
                parser
                    .set_language(&language.grammar())
                    .map_err(|error| error.to_string())?;
                Ok(entry.insert(parser))
            }
        }
    }
}

#[derive(Debug)]
struct SourceFile {
    identity: FileIdentity,
    language: Language,
    is_test: bool,
}

pub fn scan(request: &ScanRequest<'_>) -> ScanOutcome {
    let project_root = request
        .config
        .root()
        .canonicalize()
        .unwrap_or_else(|_| request.config.root().to_path_buf());
    let (mut files, mut failures) = collect_files(request.roots, &project_root);
    files.retain(|file| {
        (request.stacks.is_empty() || request.stacks.contains(&file.language.stack()))
            && !request.config.is_ignored(Path::new(&file.identity.relative))
    });
    let results: Vec<Result<Vec<Diagnostic>, ScanFailure>> = files
        .par_iter()
        .map_init(ParserPool::default, |pool, file| check_file(pool, file, request))
        .collect();
    let mut outcome = ScanOutcome {
        files: files.len(),
        ..ScanOutcome::default()
    };
    for result in results {
        match result {
            Ok(diagnostics) => outcome.diagnostics.extend(diagnostics),
            Err(failure) => failures.push(failure),
        }
    }
    outcome.diagnostics.sort_by(|left, right| {
        (&left.path, left.line, left.column, left.rule).cmp(&(
            &right.path,
            right.line,
            right.column,
            right.rule,
        ))
    });
    outcome.failures = failures;
    outcome
}

pub fn check_text(path: &Path, source: &[u8], config: &Config) -> Option<Vec<Diagnostic>> {
    let project_root = config
        .root()
        .canonicalize()
        .unwrap_or_else(|_| config.root().to_path_buf());
    let file = source_file(&project_root, path, &project_root)?;
    let relative = Path::new(&file.identity.relative);
    if config.is_ignored(relative) {
        return Some(Vec::new());
    }
    let mut parser = Parser::new();
    parser.set_language(&file.language.grammar()).ok()?;
    let analysis = analyze(&mut parser, file.language, source).ok()?;
    let limits = config.limits_for(relative, file.language, file.is_test);
    Some(evaluate(&file.identity, &analysis, limits))
}

fn collect_files(roots: &[PathBuf], project_root: &Path) -> (Vec<SourceFile>, Vec<ScanFailure>) {
    let mut files = Vec::new();
    let mut failures = Vec::new();
    for root in roots {
        for entry in WalkBuilder::new(root).build() {
            match entry {
                Ok(entry) if entry.file_type().is_some_and(|file_type| file_type.is_file()) => {
                    files.extend(source_file(root, entry.path(), project_root));
                }
                Ok(_) => {}
                Err(error) => {
                    failures.push(ScanFailure {
                        path: root.display().to_string(),
                        reason: error.to_string(),
                    });
                }
            }
        }
    }
    (files, failures)
}

fn source_file(walk_root: &Path, path: &Path, project_root: &Path) -> Option<SourceFile> {
    let language = Language::from_path(path)?;
    let is_test = language.is_test_path(path.strip_prefix(walk_root).unwrap_or(path));
    let absolute = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let relative = absolute
        .strip_prefix(project_root)
        .map_or_else(|_| portable(path), portable);
    let identity = FileIdentity {
        display: path.display().to_string(),
        relative,
        absolute,
    };
    Some(SourceFile {
        identity,
        language,
        is_test,
    })
}

fn portable(path: &Path) -> String {
    let parts: Vec<String> = path
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();
    parts.join(PATH_SEPARATOR)
}

fn check_file(
    pool: &mut ParserPool,
    file: &SourceFile,
    request: &ScanRequest<'_>,
) -> Result<Vec<Diagnostic>, ScanFailure> {
    let failure = |reason: String| ScanFailure {
        path: file.identity.display.clone(),
        reason,
    };
    let source = fs::read(&file.identity.absolute).map_err(|error| failure(error.to_string()))?;
    let parser = pool.parser(file.language).map_err(failure)?;
    let analysis = analyze(parser, file.language, &source).map_err(|error| failure(error.to_string()))?;
    let configured =
        request
            .config
            .limits_for(Path::new(&file.identity.relative), file.language, file.is_test);
    let limits = request
        .overrides
        .iter()
        .fold(configured, |limits, &limit_override| {
            limits.with_override(limit_override)
        });
    Ok(evaluate(&file.identity, &analysis, limits))
}

#[cfg(test)]
mod tests;
