use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fs;
use std::path::{Path, PathBuf};

use rayon::prelude::*;
use tree_sitter::Parser;

use crate::analysis::{FileAnalysis, ParseFailed, SourceUnit, analyze_unit};
use crate::cache::{AnalysisCache, AnalysisSubject, CacheKey};
use crate::config::Config;
use crate::duplicates::{PooledFile, find_duplicates};
use crate::i18n::Problem;
use crate::language::{Language, Stack};
use crate::limits::{LimitOverride, Limits};
use crate::macro_rules::MacroIndex;
use crate::rules::{Diagnostic, FileIdentity, RuleId, evaluate};
use crate::shell;
use crate::suppressions::{Suppressed, split};

mod project;
mod walk;

const PATH_SEPARATOR: &str = "/";
const SINGLE_FILE: usize = 1;

#[derive(Debug)]
pub struct ScanFailure {
    pub problem: Problem,
}

#[derive(Debug, Default)]
pub struct ScanOutcome {
    pub files: usize,
    pub diagnostics: Vec<Diagnostic>,
    pub suppressed: Vec<Suppressed>,
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
            Entry::Vacant(entry) => Ok(entry.insert(parser_for(language)?)),
        }
    }
}

fn parser_for(language: Language) -> Result<Parser, String> {
    let mut parser = Parser::new();
    if let Some(grammar) = language.grammar() {
        parser.set_language(&grammar).map_err(|error| error.to_string())?;
    }
    Ok(parser)
}

#[derive(Debug)]
struct SourceFile {
    identity: FileIdentity,
    language: Option<Language>,
    is_test: bool,
}

fn project_root(config: &Config) -> PathBuf {
    config
        .root()
        .canonicalize()
        .unwrap_or_else(|_| config.root().to_path_buf())
}

pub fn scan(request: &ScanRequest<'_>) -> ScanOutcome {
    let (mut files, mut failures) = collect_files(request.roots, &project_root(request.config));
    files.retain(|file| {
        let wanted = file.language.map_or(request.stacks.is_empty(), |language| {
            request.stacks.is_empty() || request.stacks.contains(&language.stack())
        });
        wanted && !request.config.is_ignored(Path::new(&file.identity.relative))
    });
    let this_scan = MacroIndex::for_one_scan();
    let macros = if files.len() == SINGLE_FILE {
        MacroIndex::shared()
    } else {
        &this_scan
    };
    let results: Vec<Result<Checked, ScanFailure>> = files
        .par_iter()
        .map_init(ParserPool::default, |pool, file| {
            check_file(pool, file, request, macros)
        })
        .collect();
    let mut outcome = ScanOutcome {
        files: files.iter().filter(|file| file.language.is_some()).count(),
        ..ScanOutcome::default()
    };
    let mut pooled = Vec::new();
    let mut indexed = Vec::new();
    for result in results {
        match result {
            Ok(checked) => {
                outcome.diagnostics.extend(checked.diagnostics);
                outcome.suppressed.extend(checked.suppressed);
                pooled.extend(checked.pooled);
                indexed.extend(checked.indexed);
            }
            Err(failure) => failures.push(failure),
        }
    }
    if let Some(cache) = request.cache {
        cache.persist();
    }
    outcome.diagnostics.extend(duplicate_diagnostics(pooled));
    if project::covers_project(request.roots, &project_root(request.config)) {
        outcome.diagnostics.extend(project::diagnostics(&indexed));
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

pub fn identities(roots: &[PathBuf], config: &Config) -> (Vec<FileIdentity>, Vec<ScanFailure>) {
    let (files, failures) = collect_files(roots, &project_root(config));
    let identities = files
        .into_iter()
        .map(|file| file.identity)
        .filter(|identity| !config.is_ignored(Path::new(&identity.relative)))
        .collect();
    (identities, failures)
}

pub fn check_text(path: &Path, source: &[u8], config: &Config) -> Option<Vec<Diagnostic>> {
    let root = project_root(config);
    let file = source_file(&root, path, &root);
    let relative = Path::new(&file.identity.relative);
    if config.is_ignored(relative) {
        return Some(Vec::new());
    }
    let limits = file
        .language
        .map(|language| config.limits_for(relative, language, file.is_test));
    let mut diagnostics = secret_scan::profile(&file.identity, config, limits)
        .map(|profile| secret_scan::diagnostics(&file.identity, &profile, source, None))
        .unwrap_or_default();
    let (Some(language), Some(limits)) = (file.language, limits) else {
        return Some(diagnostics);
    };
    let mut parser = parser_for(language).ok()?;
    let crate_macros =
        MacroIndex::shared().crate_with(language, &file.identity.absolute, config.rust_target());
    let unit = SourceUnit {
        language,
        source,
        path: Some(&file.identity.absolute),
        crate_macros: crate_macros.as_ref(),
    };
    let mut analysis = analyze_unit(&mut parser, unit, limits).ok()?;
    let pooled = pooled_file(&file.identity, &mut analysis, limits);
    diagnostics.extend(evaluate(&file.identity, &analysis, limits));
    let (mut diagnostics, _suppressed) = split(source, diagnostics);
    diagnostics.extend(find_duplicates(pooled.into_iter().collect()));
    Some(diagnostics)
}

#[derive(Debug)]
struct Checked {
    diagnostics: Vec<Diagnostic>,
    suppressed: Vec<Suppressed>,
    pooled: Option<(Stack, PooledFile)>,
    indexed: Option<project::IndexedFile>,
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
        for entry in walk::walk(root) {
            match entry {
                Ok(entry) if entry.file_type().is_some_and(|file_type| file_type.is_file()) => {
                    files.push(walked_file(root, entry.path(), project_root));
                }
                Ok(_) => {}
                Err(error) => {
                    failures.push(ScanFailure {
                        problem: Problem::Unreadable {
                            path: root.display().to_string(),
                            detail: error.to_string(),
                        },
                    });
                }
            }
        }
    }
    (files, failures)
}

fn source_file(walk_root: &Path, path: &Path, project_root: &Path) -> SourceFile {
    let language = Language::from_path(path);
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
    let is_test = language.is_some_and(|language| language.is_test_path(test_path));
    let relative = absolute
        .strip_prefix(project_root)
        .map_or_else(|_| portable(path), portable);
    let identity = FileIdentity {
        display: path.display().to_string(),
        relative,
        absolute,
    };
    SourceFile {
        identity,
        language,
        is_test,
    }
}

fn walked_file(walk_root: &Path, path: &Path, project_root: &Path) -> SourceFile {
    let file = source_file(walk_root, path, project_root);
    if !walk::is_analyzed(walk_root, path) {
        return SourceFile {
            language: None,
            ..file
        };
    }
    file
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
    let failure = |detail: String| ScanFailure {
        problem: Problem::Unreadable {
            path: file.identity.display.clone(),
            detail,
        },
    };
    let Some(language) = file.language else {
        let profile = secret_scan::profile(&file.identity, request.config, None);
        let diagnostics = secret_scan::plain_file(&file.identity, profile.as_ref(), request.cache);
        return diagnostics.map_err(failure).map(|diagnostics| Checked {
            diagnostics,
            suppressed: Vec::new(),
            pooled: None,
            indexed: None,
        });
    };
    let source = fs::read(&file.identity.absolute).map_err(|error| failure(error.to_string()))?;
    let parser = pool.parser(language).map_err(failure)?;
    let configured = request
        .config
        .limits_for(Path::new(&file.identity.relative), language, file.is_test);
    let limits = request
        .overrides
        .iter()
        .fold(configured, |limits, &limit_override| {
            limits.with_override(limit_override)
        });
    let crate_macros = macros.crate_with(language, &file.identity.absolute, request.config.rust_target());
    let context = crate_macros
        .as_ref()
        .map(|crate_macros| crate_macros.analysis_context(&file.identity.absolute))
        .or_else(|| shell::analysis_context(language, &file.identity.absolute));
    let subject = AnalysisSubject {
        language,
        source: &source,
        context,
    };
    let cached = request
        .cache
        .map(|cache| (cache, cache.analysis_key(subject, limits)));
    let unit = SourceUnit {
        language,
        source: &source,
        path: Some(&file.identity.absolute),
        crate_macros: crate_macros.as_ref(),
    };
    let mut analysis =
        cached_or_fresh(cached, || analyze_unit(parser, unit, limits)).map_err(|ParseFailed| {
            ScanFailure {
                problem: Problem::Unparsable {
                    path: file.identity.display.clone(),
                },
            }
        })?;
    let pooled = pooled_file(&file.identity, &mut analysis, limits);
    let indexed = analysis.java_summary.take().map(|summary| project::IndexedFile {
        identity: file.identity.clone(),
        limits,
        summary,
        in_tests: file.is_test,
    });
    let mut diagnostics = evaluate(&file.identity, &analysis, limits);
    if let Some(profile) = secret_scan::profile(&file.identity, request.config, Some(limits)) {
        diagnostics.extend(secret_scan::diagnostics(
            &file.identity,
            &profile,
            &source,
            request.cache,
        ));
    }
    let (diagnostics, suppressed) = split(&source, diagnostics);
    Ok(Checked {
        diagnostics,
        suppressed,
        pooled: pooled.map(|pooled| (language.stack(), pooled)),
        indexed,
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

mod secret_scan;

#[cfg(test)]
mod tests;
