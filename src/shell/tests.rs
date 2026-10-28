use std::path::Path;

use tree_sitter::Parser;

use super::{Coverage, analyze, detect, directive_coverage, disables, repair};
use crate::analysis::SourceUnit;
use crate::language::Language;
use crate::rules::{RuleId, ShellRule};

const NO_CODES: [String; 0] = [];
const NO_FINDINGS: [(String, usize, usize); 0] = [];

fn found(script: &str) -> Vec<(String, usize, usize)> {
    found_in(script, "script.sh")
}

fn found_in(script: &str, file: &str) -> Vec<(String, usize, usize)> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_bash::LANGUAGE.into())
        .expect("carga la gramática de bash");
    let unit = SourceUnit {
        language: Language::Shell,
        source: script.as_bytes(),
        path: Some(Path::new(file)),
        crate_macros: None,
        go_platform: None,
    };
    analyze(&mut parser, unit)
        .expect("es shell")
        .expect("parsea")
        .into_iter()
        .map(|violation| {
            (
                violation.rule.as_str().to_owned(),
                violation.line,
                violation.column,
            )
        })
        .collect()
}

fn codes(script: &str) -> Vec<String> {
    found(script).into_iter().map(|(code, _, _)| code).collect()
}

#[test]
fn follows_values_through_commands_loops_branches_and_functions_like_shellcheck() {
    let script = include_str!("fixtures/flow.sh");
    let expected: Vec<String> = include_str!("fixtures/flow.shellcheck")
        .lines()
        .map(str::to_owned)
        .collect();
    let actual: Vec<String> = found(script)
        .into_iter()
        .map(|(code, line, column)| format!("{line}:{column} {code}"))
        .collect();
    assert_eq!(actual, expected);
}

#[test]
fn repairs_expansions_glued_after_a_separator_before_reading_them() {
    let script = "#!/bin/bash\ncp $source/$name.txt /tmp\n";
    assert_eq!(
        found(script),
        [("sc2086".to_owned(), 2, 4), ("sc2086".to_owned(), 2, 12)]
    );
    let swallowed_lines = "#!/bin/bash\nfoo \\\n  # comment\n  $x\n";
    assert_eq!(found(swallowed_lines), NO_FINDINGS);
}

#[test]
fn reads_case_patterns_with_a_single_quoted_string_before_a_glob() {
    let script = "#!/bin/bash\ncase \"$1\" in\n  --*' '*) echo $x ;;\nesac\n";
    assert_eq!(found(script), [("sc2086".to_owned(), 3, 17)]);
    assert!(repair::double_quote_globbed_strings(b"x=a'$y'*\n").is_none());
    let requoted = repair::double_quote_globbed_strings(b"echo \"a'b'*\" # c'd'*\nx=a' '*\n")
        .expect("hay un string para cambiar");
    assert_eq!(requoted.source, b"echo \"a'b'*\" # c'd'*\nx=a\" \"*\n");
}

#[test]
fn detects_shell_scripts_by_extension_and_shebang() {
    assert_eq!(detect(Path::new("deploy.sh")), Some(Language::Shell));
    assert_eq!(detect(Path::new("lib.bash")), Some(Language::Shell));
    assert_eq!(detect(Path::new("notes.txt")), None);
    assert_eq!(detect(Path::new("no/such/extensionless")), None);
}

#[test]
fn reports_like_shellcheck_in_the_calibration_script() {
    let script = "#!/bin/bash\nk1=1; k1=2\nif [ $? -ne 0 ]; then :; fi\n[ \"$a\" -a \"$b\" ]\necho foo && echo bar || cat\ncd /tmp\necho `date`\necho $(echo $x)\nq=( $(ls) $v )\necho 2>&1 >/dev/null\ntrap \"echo $a\" EXIT\nf() { local r=$(date); }\necho 'a $b'\nx=$(( $n + 1 ))\n";
    let expected: Vec<(String, usize, usize)> = [
        ("sc2034", 2, 7),
        ("sc2181", 3, 6),
        ("sc2166", 4, 8),
        ("sc2015", 5, 10),
        ("sc2164", 6, 1),
        ("sc2005", 7, 6),
        ("sc2006", 7, 6),
        ("sc2046", 7, 6),
        ("sc2005", 8, 6),
        ("sc2046", 8, 6),
        ("sc2116", 8, 6),
        ("sc2086", 8, 13),
        ("sc2034", 9, 1),
        ("sc2207", 9, 5),
        ("sc2206", 9, 11),
        ("sc2069", 10, 6),
        ("sc2064", 11, 12),
        ("sc2034", 12, 13),
        ("sc2155", 12, 13),
        ("sc2016", 13, 6),
        ("sc2004", 14, 7),
    ]
    .into_iter()
    .map(|(code, line, column)| (code.to_owned(), line, column))
    .collect();
    let mut actual = found(script);
    actual.sort();
    let mut expected = expected;
    expected.sort();
    assert_eq!(actual, expected);
}

#[test]
fn needs_a_shebang_unless_the_extension_says_the_shell() {
    assert_eq!(codes("echo hola\n"), ["sc2148"]);
    assert_eq!(found_in("echo hola\n", "lib.bash"), NO_FINDINGS);
    assert_eq!(codes("# shellcheck shell=bash\necho hola\n"), NO_CODES);
}

#[test]
fn directives_cover_the_next_statement_or_the_whole_file_like_shellcheck() {
    let script =
        "#!/bin/bash\necho\n# shellcheck disable=SC2086\n# reason\nif x; then\n  echo $a\nfi\necho $b\n";
    assert_eq!(directive_coverage(script).get(&3), Some(&Coverage::Lines(5, 7)));
    let file_wide = "#!/bin/bash\n# shellcheck disable=SC2086,SC2034\nx=1\n";
    assert_eq!(directive_coverage(file_wide).get(&2), Some(&Coverage::File));
    let trailing = "#!/bin/bash\necho $a # shellcheck disable=SC2086\necho\n";
    assert!(directive_coverage(trailing).is_empty());
    let before_else =
        "#!/bin/bash\necho\nif x; then\n  :\n# shellcheck disable=SC2086\nelse\n  echo $a\nfi\n";
    assert!(directive_coverage(before_else).is_empty());
    let rule = RuleId::Shell(ShellRule::Sc2086);
    assert!(disables("SC2086", rule) && disables("2086", rule) && disables("all", rule));
    assert!(disables("SC2000-SC2099", rule) && !disables("SC2087-SC2099", rule) && !disables("SC2034", rule));
    assert!(!disables("all", RuleId::FileLength));
}

#[test]
fn quotes_only_what_may_contain_spaces() {
    assert_eq!(codes("#!/bin/bash\na=foo\necho $a\n"), NO_CODES);
    assert_eq!(codes("#!/bin/bash\na='cow moo'\necho $a\n"), ["sc2086"]);
    assert_eq!(codes("#!/bin/bash\nfor i in 1 2 3; do echo $i; done\n"), NO_CODES);
    assert_eq!(
        codes("#!/bin/bash\nfor i in 1 2 *; do echo $i; done\n"),
        ["sc2086"]
    );
    assert_eq!(codes("#!/bin/bash\nx=0; (( x += 1 )); echo $x\n"), NO_CODES);
    assert_eq!(codes("#!/bin/bash\ndeclare -i s=0; s=$(f); echo $s\n"), NO_CODES);
    assert_eq!(
        codes("#!/bin/bash\nx=1; if f; then x='foo bar'; exit; fi; echo $x\n"),
        NO_CODES
    );
    assert_eq!(
        codes("#!/bin/bash\ns=1; f() { local s='a b'; }; f; echo $s\n"),
        NO_CODES
    );
    assert_eq!(
        codes("#!/bin/bash\ns='a b'; f() { s=1; }; f; echo $s\n"),
        NO_CODES
    );
    assert_eq!(
        codes("#!/bin/bash\ns='a b'; x && f() { s=1; }; f; echo $s\n"),
        ["sc2086"]
    );
    assert_eq!(codes("#!/bin/bash\nf() { s=$?; echo $s; }\n"), NO_CODES);
    assert_eq!(codes("#!/bin/bash\necho foo > $file\n"), ["sc2086"]);
    assert_eq!(codes("#!/bin/bash\n(( $1 + 3 ))\n"), NO_CODES);
}
