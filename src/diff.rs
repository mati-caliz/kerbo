use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::rules::Diagnostic;

const NEW_FILE_PREFIX: &str = "+++ ";
const NEW_FILE_SIDE: &str = "b/";
const DELETED_FILE: &str = "/dev/null";
const HUNK_PREFIX: &str = "@@ ";

#[derive(Debug, Clone, PartialEq, Eq)]
enum Changes {
    Whole,
    Ranges(Vec<(usize, usize)>),
}

#[derive(Debug, Default)]
pub struct ChangedLines {
    files: HashMap<PathBuf, Changes>,
}

impl ChangedLines {
    pub fn since(reference: &str, directory: &Path) -> Result<Self, String> {
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
        let mut files = parse_unified_diff(&diff, &root);
        for path in untracked.split('\0').filter(|path| !path.is_empty()) {
            files.insert(root.join(path), Changes::Whole);
        }
        Ok(Self { files })
    }

    pub fn touches(&self, diagnostic: &Diagnostic) -> bool {
        match self.files.get(&diagnostic.absolute_path) {
            None => false,
            Some(Changes::Whole) => true,
            Some(Changes::Ranges(ranges)) => ranges
                .iter()
                .any(|&(first, last)| first <= diagnostic.end_line && diagnostic.line <= last),
        }
    }
}

fn git(directory: &Path, arguments: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(directory)
        .args(arguments)
        .output()
        .map_err(|error| format!("no se pudo correr git: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git {}: {}",
            arguments.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn parse_unified_diff(text: &str, root: &Path) -> HashMap<PathBuf, Changes> {
    let mut files: HashMap<PathBuf, Changes> = HashMap::new();
    let mut current: Option<PathBuf> = None;
    for line in text.lines() {
        if let Some(target) = line.strip_prefix(NEW_FILE_PREFIX) {
            current = (target != DELETED_FILE)
                .then(|| root.join(target.strip_prefix(NEW_FILE_SIDE).unwrap_or(target)));
            continue;
        }
        let (Some(file), Some(range)) = (&current, line.strip_prefix(HUNK_PREFIX).and_then(new_side_range))
        else {
            continue;
        };
        if let Changes::Ranges(ranges) = files
            .entry(file.clone())
            .or_insert_with(|| Changes::Ranges(Vec::new()))
        {
            ranges.push(range);
        }
    }
    files
}

fn new_side_range(hunk: &str) -> Option<(usize, usize)> {
    let new_side = hunk.split_whitespace().find_map(|part| part.strip_prefix('+'))?;
    let (start, count) = new_side.split_once(',').unwrap_or((new_side, "1"));
    let start: usize = start.parse().ok()?;
    let count: usize = count.parse().ok()?;
    let first = start.max(1);
    Some((first, first.saturating_add(count.saturating_sub(1))))
}

#[cfg(test)]
mod tests;
