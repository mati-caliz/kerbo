use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fs;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;
use rayon::prelude::*;
use tree_sitter::Parser;

use crate::analysis::analyze;
use crate::language::{Language, Stack};
use crate::rules::{Diagnostic, Limits, evaluate};

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

pub fn scan(roots: &[PathBuf], stacks: &[Stack]) -> ScanOutcome {
    let (mut files, mut failures) = collect_files(roots);
    files.retain(|file| stacks.is_empty() || stacks.contains(&file.language.stack()));
    let results: Vec<Result<Vec<Diagnostic>, ScanFailure>> = files
        .par_iter()
        .map_init(ParserPool::default, check_file)
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

#[derive(Debug)]
struct SourceFile {
    path: PathBuf,
    language: Language,
    is_test: bool,
}

fn collect_files(roots: &[PathBuf]) -> (Vec<SourceFile>, Vec<ScanFailure>) {
    let mut files = Vec::new();
    let mut failures = Vec::new();
    for root in roots {
        for entry in WalkBuilder::new(root).build() {
            match entry {
                Ok(entry) if entry.file_type().is_some_and(|file_type| file_type.is_file()) => {
                    files.extend(source_file(root, entry.into_path()));
                }
                Ok(_) => {}
                Err(error) => failures.push(ScanFailure {
                    path: root.display().to_string(),
                    reason: error.to_string(),
                }),
            }
        }
    }
    (files, failures)
}

fn source_file(root: &Path, path: PathBuf) -> Option<SourceFile> {
    let language = Language::from_path(&path)?;
    let is_test = language.is_test_path(path.strip_prefix(root).unwrap_or(&path));
    Some(SourceFile {
        path,
        language,
        is_test,
    })
}

fn check_file(pool: &mut ParserPool, file: &SourceFile) -> Result<Vec<Diagnostic>, ScanFailure> {
    let display_path = file.path.display().to_string();
    let failure = |reason: String| ScanFailure {
        path: display_path.clone(),
        reason,
    };
    let source = fs::read(&file.path).map_err(|error| failure(error.to_string()))?;
    let parser = pool.parser(file.language).map_err(failure)?;
    let analysis = analyze(parser, file.language, &source).map_err(|error| failure(error.to_string()))?;
    Ok(evaluate(
        &display_path,
        &analysis,
        Limits::strict_for(file.language, file.is_test),
    ))
}

#[cfg(test)]
mod tests {
    use super::scan;
    use crate::language::Stack;
    use crate::rules::RuleId;
    use crate::test_support::TempTree;

    #[test]
    fn analyzes_supported_files_in_parallel_and_sorts_the_problems() {
        let tree = TempTree::new("scan-mixed");
        tree.write("b.py", "def f(a, b, c, d, e, f, g):\n    pass\n");
        tree.write("a/x.ts", "function f(a, b, c, d, e) {}\n");
        tree.write("notes.md", "# no es código\n");
        let outcome = scan(&[tree.path().to_path_buf()], &[]);
        assert_eq!(outcome.files, 2);
        assert!(outcome.failures.is_empty());
        let found: Vec<(bool, RuleId)> = outcome
            .diagnostics
            .iter()
            .map(|diagnostic| (diagnostic.path.ends_with("x.ts"), diagnostic.rule))
            .collect();
        assert_eq!(
            found,
            [(true, RuleId::ParameterCount), (false, RuleId::ParameterCount)]
        );
    }

    #[test]
    fn accepts_several_roots_and_single_files() {
        let tree = TempTree::new("scan-roots");
        tree.write("one/A.java", "class A { void f() {} }\n");
        tree.write("two.go", "package two\n");
        let outcome = scan(&[tree.path().join("one"), tree.path().join("two.go")], &[]);
        assert_eq!(outcome.files, 2);
        assert!(outcome.diagnostics.is_empty());
        assert!(scan(&[], &[]).diagnostics.is_empty());
        let only_go = scan(&[tree.path().to_path_buf()], &[Stack::Go]);
        assert_eq!(only_go.files, 1);
    }
}
