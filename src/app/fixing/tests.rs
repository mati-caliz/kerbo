use std::fs;
use std::path::Path;

use std::process::Command;

use super::{FixRequest, Fixer, converge, fix_files, write_atomically};
use crate::app::{EXIT_CLEAN, Terminal, run};
use crate::cli::try_parse;
use crate::config::Config;
use crate::fix::{Fix, TextEdit, TextPoint};
use crate::i18n::{Lang, LangPreference};
use crate::language::Language;
use crate::rules::Diagnostic;
use crate::rules::RuleId;
use crate::scan::check_text;
use crate::test_support::TempTree;

const GREETER: &str = "import java.util.List;\nimport java.util.Map;\n\nclass Greeter {\n    void greet(String name) {\n        System.out.println(name);\n    }\n}\n";
const SINGLETON: &str = "class Single {\n    private Single() { }\n\n    final void run(final int times) {\n        run(times);\n    }\n}\n";

fn project(name: &str, file: &str, contents: &str) -> TempTree {
    let tree = TempTree::new(name);
    tree.write(file, contents);
    tree
}

fn request<'request>(tree: &TempTree, rules: &'request [RuleId], write: bool) -> FixRequest<'request> {
    FixRequest {
        paths: vec![tree.path().to_path_buf()],
        languages: &[],
        rules,
        config: None,
        write,
        since: None,
        unsafe_fixes: false,
    }
}

fn read(tree: &TempTree, file: &str) -> String {
    fs::read_to_string(tree.path().join(file)).expect("el archivo se lee")
}

#[test]
fn a_dry_run_shows_the_whole_patch_and_leaves_the_file_alone() {
    let tree = project("fix-dry", "Greeter.java", GREETER);
    let outcome = fix_files(&request(&tree, &[], false), tree.path()).expect("se analiza");
    assert_eq!(read(&tree, "Greeter.java"), GREETER);
    assert_eq!(
        outcome.patch(),
        "--- a/Greeter.java\n+++ b/Greeter.java\n@@ -1,2 +0,0 @@\n-import java.util.List;\n-import java.util.Map;\n@@ -5 +3 @@\n-    void greet(String name) {\n+    void greet(final String name) {\n"
    );
    assert_eq!(
        outcome.notices(Lang::Spanish),
        ["3 arreglos en 1 archivo; no se escribió nada"]
    );
}

#[test]
fn writes_every_compatible_fix_and_follows_the_ones_that_appear_after() {
    let tree = project("fix-write", "Single.java", SINGLETON);
    let outcome = fix_files(&request(&tree, &[], true), tree.path()).expect("se analiza");
    assert_eq!(
        read(&tree, "Single.java"),
        "final class Single {\n    private Single() { }\n\n    void run(final int times) {\n        run(times);\n    }\n}\n"
    );
    assert_eq!(outcome.files.first().map(|file| file.applied), Some(2));
    assert!(outcome.withheld().next().is_none());
    assert_eq!(outcome.notices(Lang::English), ["2 fixes applied in 1 file"]);
    let again = fix_files(&request(&tree, &[], true), tree.path()).expect("se analiza");
    assert!(again.files.is_empty());
}

#[test]
fn applies_only_the_rules_asked_for() {
    let tree = project("fix-rule", "Greeter.java", GREETER);
    fix_files(&request(&tree, &[RuleId::UnusedImport], true), tree.path()).expect("se analiza");
    assert_eq!(
        read(&tree, "Greeter.java"),
        "\nclass Greeter {\n    void greet(String name) {\n        System.out.println(name);\n    }\n}\n"
    );
}

#[test]
fn the_command_prints_the_patch_on_a_dry_run() {
    let tree = project("fix-command", "Greeter.java", GREETER);
    let path = tree.path().to_str().expect("la ruta es UTF-8");
    let cli = try_parse(
        Lang::Spanish,
        ["kerbo", "fix", "--diff", "--rule", "final-parameter", path],
    )
    .expect("los argumentos son válidos");
    let (mut report, mut log) = (Vec::new(), Vec::new());
    let status = run(
        &cli.command,
        LangPreference {
            flag: None,
            locale: Lang::Spanish,
        },
        tree.path(),
        &mut Terminal {
            input: &mut &b""[..],
            report: &mut report,
            log: &mut log,
        },
    );
    assert_eq!(status, EXIT_CLEAN);
    assert!(String::from_utf8_lossy(&report).contains("+    void greet(final String name) {"));
    assert!(String::from_utf8_lossy(&log).contains("1 arreglo en 1 archivo"));
    assert_eq!(read(&tree, "Greeter.java"), GREETER);
}

#[test]
fn an_atomic_write_replaces_the_file_and_leaves_nothing_behind() {
    let tree = project("fix-atomic", "a.txt", "antes\n");
    let path = tree.path().join("a.txt");
    write_atomically(&path, "después\n").expect("se escribe");
    assert_eq!(read(&tree, "a.txt"), "después\n");
    let entries = fs::read_dir(tree.path()).expect("se lista").count();
    assert_eq!(entries, 1);
    assert!(write_atomically(Path::new("/no/existe/a.txt"), "x").is_err());
}

fn git(tree: &TempTree, arguments: &[&str]) {
    let status = Command::new("git")
        .args([
            "-c",
            "user.name=kerbo",
            "-c",
            "user.email=kerbo@test",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(arguments)
        .current_dir(tree.path())
        .output()
        .expect("git corre")
        .status;
    assert!(status.success(), "git {arguments:?}");
}

#[test]
fn since_fixes_only_what_the_change_touches() {
    let tree = project("fix-since", "Greeter.java", GREETER);
    git(&tree, &["init", "-q"]);
    git(&tree, &["add", "."]);
    git(&tree, &["commit", "-q", "-m", "inicio"]);
    let edited = GREETER
        .replace("void greet(String name)", "void greet(String who)")
        .replace("println(name)", "println(who)");
    tree.write("Greeter.java", &edited);
    let mut since = request(&tree, &[], true);
    since.since = Some("HEAD");
    fix_files(&since, tree.path()).expect("se analiza");
    assert_eq!(
        read(&tree, "Greeter.java"),
        edited.replace("void greet(String who)", "void greet(final String who)"),
        "los imports sin usar no se tocaron: no son parte del cambio"
    );
}

#[test]
fn a_pass_that_breaks_the_syntax_is_not_accepted() {
    let tree = project("fix-syntax", "Broken.java", GREETER);
    let config = Config::load(None, tree.path()).expect("sin configuración");
    let path = tree.path().join("Broken.java");
    let before = check_text(&path, GREETER.as_bytes(), &config).expect("se analiza");
    let breaking: Vec<Diagnostic> = before
        .into_iter()
        .take(1)
        .map(|mut diagnostic| {
            diagnostic.fix = Some(Fix {
                edits: vec![TextEdit {
                    start: TextPoint { line: 8, column: 1 },
                    end: TextPoint { line: 8, column: 2 },
                    replacement: String::new(),
                }],
                hunk: String::new(),
                safe: true,
                isolation: None,
            });
            diagnostic
        })
        .collect();
    let analyze = |text: &str| check_text(&path, text.as_bytes(), &config);
    let accepts = |diagnostic: &Diagnostic| diagnostic.fix.is_some();
    let fixer = Fixer {
        language: Language::from_path(&path),
        analyze: &analyze,
        accepts: &accepts,
    };
    assert!(
        converge(&fixer, GREETER, breaking).is_none(),
        "sacar la llave final rompe la clase"
    );
}
