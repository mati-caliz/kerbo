use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fs;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;
use rayon::prelude::*;
use tree_sitter::Parser;

use crate::analysis::{FileAnalysis, ParseFailed, SourceUnit, analyze_unit};
use crate::cache::{AnalysisCache, AnalysisSubject, CacheKey};
use crate::config::Config;
use crate::duplicates::{PooledFile, find_duplicates};
use crate::language::{Language, Stack};
use crate::limits::{LimitOverride, Limits};
use crate::macro_rules::MacroIndex;
use crate::rules::{Diagnostic, FileIdentity, RuleId, evaluate};

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
    pub cache: Option<&'request AnalysisCache>,
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
    let macros = MacroIndex::for_one_scan();
    let results: Vec<Result<Checked, ScanFailure>> = files
        .par_iter()
        .map_init(ParserPool::default, |pool, file| {
            check_file(pool, file, request, &macros)
        })
        .collect();
    let mut outcome = ScanOutcome {
        files: files.len(),
        ..ScanOutcome::default()
    };
    let mut pooled = Vec::new();
    for result in results {
        match result {
            Ok(checked) => {
                outcome.diagnostics.extend(checked.diagnostics);
                pooled.extend(checked.pooled);
            }
            Err(failure) => failures.push(failure),
        }
    }
    if let Some(cache) = request.cache {
        cache.persist();
    }
    outcome.diagnostics.extend(duplicate_diagnostics(pooled));
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
    let limits = config.limits_for(relative, file.language, file.is_test);
    let crate_macros = MacroIndex::shared().crate_of(file.language, &file.identity.absolute);
    let unit = SourceUnit {
        language: file.language,
        source,
        path: Some(&file.identity.absolute),
        crate_macros: crate_macros.as_ref(),
    };
    let mut analysis = analyze_unit(&mut parser, unit, limits).ok()?;
    let pooled = pooled_file(&file.identity, &mut analysis, limits);
    let mut diagnostics = evaluate(&file.identity, &analysis, limits);
    diagnostics.extend(find_duplicates(pooled.into_iter().collect()));
    Some(diagnostics)
}

#[derive(Debug)]
struct Checked {
    diagnostics: Vec<Diagnostic>,
    pooled: Option<(Stack, PooledFile)>,
}

fn pooled_file(identity: &FileIdentity, analysis: &mut FileAnalysis, limits: Limits) -> Option<PooledFile> {
    Some(PooledFile {
        identity: identity.clone(),
        limit: limits.get(RuleId::DuplicateCode)?,
        units: analysis.duplicate_units.take()?,
    })
}

type PoolScope = (Stack, Option<PathBuf>);

fn duplicate_diagnostics(pooled: Vec<(Stack, PooledFile)>) -> Vec<Diagnostic> {
    let mut by_scope: Vec<(PoolScope, Vec<PooledFile>)> = Vec::new();
    for (stack, file) in pooled {
        let scope = (stack, file.package_scope().map(Path::to_path_buf));
        match by_scope.iter_mut().find(|(known, _)| *known == scope) {
            Some((_, files)) => files.push(file),
            None => by_scope.push((scope, vec![file])),
        }
    }
    by_scope
        .into_iter()
        .flat_map(|(_, files)| find_duplicates(files))
        .collect()
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
    let absolute = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let test_path = absolute
        .strip_prefix(project_root)
        .ok()
        .or_else(|| {
            path.strip_prefix(walk_root)
                .ok()
                .filter(|relative| !relative.as_os_str().is_empty())
        })
        .unwrap_or(path);
    let is_test = language.is_test_path(test_path);
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
    macros: &MacroIndex,
) -> Result<Checked, ScanFailure> {
    let failure = |reason: String| ScanFailure {
        path: file.identity.display.clone(),
        reason,
    };
    let source = fs::read(&file.identity.absolute).map_err(|error| failure(error.to_string()))?;
    let parser = pool.parser(file.language).map_err(failure)?;
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
    let crate_macros = macros.crate_of(file.language, &file.identity.absolute);
    let context = crate_macros
        .as_ref()
        .map(|crate_macros| crate_macros.analysis_context(&file.identity.absolute));
    let subject = AnalysisSubject {
        language: file.language,
        source: &source,
        context,
    };
    let cached = request
        .cache
        .map(|cache| (cache, cache.analysis_key(subject, limits)));
    let unit = SourceUnit {
        language: file.language,
        source: &source,
        path: Some(&file.identity.absolute),
        crate_macros: crate_macros.as_ref(),
    };
    let mut analysis = cached_or_fresh(cached, || analyze_unit(parser, unit, limits))
        .map_err(|error| failure(error.to_string()))?;
    let pooled = pooled_file(&file.identity, &mut analysis, limits);
    Ok(Checked {
        diagnostics: evaluate(&file.identity, &analysis, limits),
        pooled: pooled.map(|pooled| (file.language.stack(), pooled)),
    })
}

fn cached_or_fresh(
    cached: Option<(&AnalysisCache, CacheKey)>,
    fresh: impl FnOnce() -> Result<FileAnalysis, ParseFailed>,
) -> Result<FileAnalysis, ParseFailed> {
    if let Some(analysis) = cached.and_then(|(cache, key)| cache.analysis(key)) {
        return Ok(analysis);
    }
    let analysis = fresh()?;
    if let Some((cache, key)) = cached {
        cache.remember(key, &analysis);
    }
    Ok(analysis)
}

#[cfg(test)]
mod tests;
