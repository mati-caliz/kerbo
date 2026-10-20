use std::path::{Path, PathBuf};
use std::process::Command;

use super::{ChangedLines, Changes, LineRange, new_side_range, parse_unified_diff};
use crate::i18n::Message;
use crate::rules::{Diagnostic, RuleId};
use crate::test_support::TempTree;

const DIFF: &str = "diff --git a/src/a.ts b/src/a.ts
index 1..2 100644
--- a/src/a.ts
+++ b/src/a.ts
@@ -10,0 +11,3 @@ function f() {
+  if (a) {}
@@ -40 +44 @@
-x
+y
diff --git a/gone.ts b/gone.ts
--- a/gone.ts
+++ /dev/null
@@ -1,3 +0,0 @@
";

fn diagnostic(absolute: &Path, line: usize, end_line: usize) -> Diagnostic {
    Diagnostic {
        path: String::new(),
        line,
        column: 1,
        end_line,
        rule: RuleId::FunctionLength,
        message: Message::Rule(RuleId::FunctionLength),
        value: 1,
        limit: 0,
        fingerprint: String::new(),
        relative_path: String::new(),
        absolute_path: absolute.to_path_buf(),
        fix: None,
    }
}

const fn range(first: usize, last: usize, added: bool) -> LineRange {
    LineRange { first, last, added }
}

#[test]
fn reads_the_new_side_of_each_hunk() {
    assert_eq!(
        new_side_range("-10,0 +11,3 @@ function f() {"),
        Some(range(11, 13, true))
    );
    assert_eq!(new_side_range("-40 +44 @@"), Some(range(44, 44, true)));
    assert_eq!(
        new_side_range("-5,2 +4,0 @@"),
        Some(range(4, 4, false)),
        "un borrado marca la línea donde estaba"
    );
    assert_eq!(new_side_range("-1,3 +0,0 @@"), Some(range(1, 1, false)));
    assert_eq!(new_side_range("sin rangos"), None);
    let parsed = parse_unified_diff(DIFF, Path::new("/repo"));
    let changes = parsed
        .files
        .get(&PathBuf::from("/repo/src/a.ts"))
        .expect("a.ts cambió");
    assert_eq!(
        changes,
        &Changes::Ranges(vec![range(11, 13, true), range(44, 44, true)])
    );
    assert!(changes.adds_line(12) && !changes.adds_line(20) && Changes::Whole.adds_line(7));
    assert!(
        !Changes::Ranges(vec![range(4, 4, false)]).adds_line(4),
        "un borrado no agrega líneas"
    );
    assert_eq!(
        parsed.files.len(),
        1,
        "los archivos borrados no cuentan como cambiados"
    );
    assert_eq!(parsed.deleted, [PathBuf::from("/repo/gone.ts")]);
}

#[test]
fn keeps_only_problems_that_touch_changed_lines() {
    let changes = ChangedLines {
        files: parse_unified_diff(DIFF, Path::new("/repo")).files,
        ..ChangedLines::default()
    };
    let file = Path::new("/repo/src/a.ts");
    assert!(changes.touches(&diagnostic(file, 5, 12)));
    assert!(changes.touches(&diagnostic(file, 44, 60)));
    assert!(!changes.touches(&diagnostic(file, 14, 43)));
    assert!(!changes.touches(&diagnostic(Path::new("/repo/other.ts"), 1, 99)));
}

fn git(directory: &Path, arguments: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(directory)
        .args(["-c", "user.name=kerbo", "-c", "user.email=kerbo@example.com"])
        .args(arguments)
        .status()
        .expect("git está instalado");
    assert!(status.success(), "git {arguments:?}");
}

#[test]
fn asks_git_for_changes_untracked_files_and_previous_versions() {
    let tree = TempTree::new("diff-git");
    git(tree.path(), &["init", "-q"]);
    tree.write("a.ts", "line 1\nline 2\nline 3\n");
    git(tree.path(), &["add", "a.ts"]);
    git(tree.path(), &["commit", "-q", "-m", "base"]);
    tree.write("a.ts", "line 1\nchanged\nline 3\n");
    tree.write("new.ts", "fresh\n");
    let changes = ChangedLines::since("HEAD", tree.path()).expect("git responde");
    let root = tree.path().canonicalize().expect("existe");
    assert!(changes.touches(&diagnostic(&root.join("a.ts"), 2, 2)));
    assert!(!changes.touches(&diagnostic(&root.join("a.ts"), 3, 3)));
    assert!(changes.touches(&diagnostic(&root.join("new.ts"), 1, 1)));
    assert_eq!(
        changes.previous_version(&root.join("a.ts")).as_deref(),
        Some("line 1\nline 2\nline 3\n")
    );
    assert!(changes.previous_version(&root.join("new.ts")).is_none());
    assert!(changes.previous_version(Path::new("/elsewhere.ts")).is_none());
    assert_eq!(changes.files().count(), 2);
    assert_eq!(changes.deleted().len(), 0);
    assert!(ChangedLines::since("no-such-ref", tree.path()).is_err());
    assert!(ChangedLines::since("HEAD", &root.join("missing")).is_err());
}
