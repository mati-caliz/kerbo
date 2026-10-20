use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::i18n::Problem;
use crate::rules::Diagnostic;

const NEW_FILE_PREFIX: &str = "+++ ";
const OLD_FILE_PREFIX: &str = "--- ";
const NEW_FILE_SIDE: &str = "b/";
const OLD_FILE_SIDE: &str = "a/";
const DELETED_FILE: &str = "/dev/null";
const HUNK_PREFIX: &str = "@@ ";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineRange {
    pub first: usize,
    pub last: usize,
    pub added: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Changes {
    Whole,
    Ranges(Vec<LineRange>),
}

#[derive(Debug, Default)]
pub struct ChangedLines {
    root: PathBuf,
    reference: String,
    files: HashMap<PathBuf, Changes>,
    deleted: Vec<PathBuf>,
}

#[derive(Debug, Default)]
struct ParsedDiff {
    files: HashMap<PathBuf, Changes>,
    deleted: Vec<PathBuf>,
}

impl ChangedLines {
    pub fn since(reference: &str, directory: &Path) -> Result<Self, Problem> {
        let root = PathBuf::from(git(directory, &["rev-parse", "--show-toplevel"])?.trim());
        let diff = git(
            &root,
            &[
                "diff",
                "--unified=0",
                "--no-color",
                "--no-ext-diff",
                reference,
                "--",
            ],
        )?;
        let untracked = git(&root, &["ls-files", "--others", "--exclude-standard", "-z"])?;
        let ParsedDiff { mut files, deleted } = parse_unified_diff(&diff, &root);
        for path in untracked.split('\0').filter(|path| !path.is_empty()) {
            files.insert(root.join(path), Changes::Whole);
        }
        Ok(Self {
            root,
            reference: reference.to_owned(),
            files,
            deleted,
        })
    }

    pub fn touches(&self, diagnostic: &Diagnostic) -> bool {
        match self.files.get(&diagnostic.absolute_path) {
            None => false,
            Some(Changes::Whole) => true,
            Some(Changes::Ranges(ranges)) => ranges
                .iter()
                .any(|range| range.first <= diagnostic.end_line && diagnostic.line <= range.last),
        }
    }

    pub fn files(&self) -> impl Iterator<Item = (&Path, &Changes)> {
        self.files.iter().map(|(path, changes)| (path.as_path(), changes))
    }

    pub fn deleted(&self) -> &[PathBuf] {
        &self.deleted
    }

    pub fn previous_version(&self, path: &Path) -> Option<String> {
        let relative = path.strip_prefix(&self.root).ok()?.to_str()?;
        git(&self.root, &["show", &format!("{}:{relative}", self.reference)]).ok()
    }
}

impl Changes {
    pub fn touches_span(&self, first: usize, last: usize) -> bool {
        match self {
            Self::Whole => true,
            Self::Ranges(ranges) => ranges
                .iter()
                .any(|range| range.first <= last && first <= range.last),
        }
    }

    pub fn adds_line(&self, line: usize) -> bool {
        match self {
            Self::Whole => true,
            Self::Ranges(ranges) => ranges
                .iter()
                .any(|range| range.added && range.first <= line && line <= range.last),
        }
    }
}

fn git(directory: &Path, arguments: &[&str]) -> Result<String, Problem> {
    let output = Command::new("git")
        .arg("-C")
        .arg(directory)
        .args(arguments)
        .output()
        .map_err(|error| Problem::GitUnavailable(error.to_string()))?;
    if !output.status.success() {
        return Err(Problem::GitFailed {
            command: arguments.join(" "),
            detail: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn parse_unified_diff(text: &str, root: &Path) -> ParsedDiff {
    let mut parsed = ParsedDiff::default();
    let mut previous: Option<PathBuf> = None;
    let mut current: Option<PathBuf> = None;
    for line in text.lines() {
        if let Some(source) = line.strip_prefix(OLD_FILE_PREFIX) {
            previous = (source != DELETED_FILE)
                .then(|| root.join(source.strip_prefix(OLD_FILE_SIDE).unwrap_or(source)));
            continue;
        }
        if let Some(target) = line.strip_prefix(NEW_FILE_PREFIX) {
            current = (target != DELETED_FILE)
                .then(|| root.join(target.strip_prefix(NEW_FILE_SIDE).unwrap_or(target)));
            if current.is_none() {
                parsed.deleted.extend(previous.take());
            }
            continue;
        }
        let (Some(file), Some(range)) = (&current, line.strip_prefix(HUNK_PREFIX).and_then(new_side_range))
        else {
            continue;
        };
        if let Changes::Ranges(ranges) = parsed
            .files
            .entry(file.clone())
            .or_insert_with(|| Changes::Ranges(Vec::new()))
        {
            ranges.push(range);
        }
    }
    parsed
}

fn new_side_range(hunk: &str) -> Option<LineRange> {
    let new_side = hunk.split_whitespace().find_map(|part| part.strip_prefix('+'))?;
    let (start, count) = new_side.split_once(',').unwrap_or((new_side, "1"));
    let start: usize = start.parse().ok()?;
    let count: usize = count.parse().ok()?;
    let first = start.max(1);
    Some(LineRange {
        first,
        last: first.saturating_add(count.saturating_sub(1)),
        added: count > 0,
    })
}

#[cfg(test)]
mod tests;
