use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::keep_confirmed;
use crate::go_types::ModuleFacts;
use crate::i18n::Message;
use crate::lint::{LintFinding, LintKind};
use crate::rules::{Diagnostic, FileIdentity, GoRule, finding_diagnostics};
use crate::suppressions::Suppressed;
use crate::test_support::TempTree;

const FACTS: &str = r#""calls":[
    {"line":3,"column":9,"names":["(*os.File).Close"],"argument":"","function":"(*os.File).Close","selector":"file.Close"},
    {"line":4,"column":13,"names":["fmt.Println"],"argument":"string","function":"fmt.Println","selector":"fmt.Println"},
    {"line":5,"column":13,"names":["fmt.Fprintf"],"argument":"*strings.Builder","function":"fmt.Fprintf","selector":"fmt.Fprintf"},
    {"line":6,"column":8,"names":null,"argument":"","function":"","selector":""},
    {"line":7,"column":11,"names":["errors.AsType"],"argument":"","errors_as_type":true,"function":"errors.AsType","selector":"errors.AsType"}
  ],
  "receives":[{"line":8,"column":2}],
  "ranges":[
    {"line":9,"column":2,"call":null},
    {"line":10,"column":2,"call":{"line":10,"column":20,"names":["(hash.Hash).Write"],"function":"(hash.Hash).Write","selector":"h.Write"}}
  ],
  "embedded":[{"line":13,"column":8}],
  "fprints":[{"line":14,"column":2,"replacement":"fmt.Fprintf"}]"#;

fn candidates(identity: &FileIdentity, positions: &[(usize, usize)]) -> Vec<Diagnostic> {
    typed_candidates(identity, GoRule::Errcheck, positions, "")
}

fn typed_candidates(
    identity: &FileIdentity,
    rule: GoRule,
    positions: &[(usize, usize)],
    subject: &str,
) -> Vec<Diagnostic> {
    let found: Vec<LintFinding> = positions
        .iter()
        .map(|(line, column)| LintFinding {
            kind: LintKind::Go(rule),
            line: *line,
            column: *column,
            subject: subject.to_owned(),
            fix: None,
        })
        .collect();
    finding_diagnostics(identity, &found).collect()
}

fn subjects(diagnostics: &[Diagnostic]) -> Vec<String> {
    diagnostics
        .iter()
        .map(|diagnostic| {
            let Message::Lint { subject, .. } = &diagnostic.message else {
                return String::new();
            };
            format!("{}:{} {subject}", diagnostic.line, diagnostic.column)
        })
        .collect()
}

#[test]
fn candidates_stay_only_where_the_types_say_an_error_is_dropped() {
    let tree = TempTree::new("go-errcheck-confirm");
    tree.write("go.mod", "module a\n");
    tree.write("a.go", "package a\n");
    let file: PathBuf = tree.path().join("a.go");
    let identity = FileIdentity {
        display: "a.go".to_owned(),
        relative: "a.go".to_owned(),
        absolute: file.clone(),
    };
    let line = format!(
        "{{\"file\":{},{FACTS}}}",
        serde_json::to_string(&file.display().to_string()).expect("la ruta es texto")
    )
    .replace('\n', "");
    let modules: HashMap<&Path, ModuleFacts> = HashMap::from([(tree.path(), ModuleFacts::parse(&line))]);
    let roots = HashMap::from([(file, tree.path().to_path_buf())]);
    let positions = [
        (3, 9),
        (4, 13),
        (5, 13),
        (6, 8),
        (7, 11),
        (8, 2),
        (9, 2),
        (10, 2),
        (11, 1),
    ];
    let mut diagnostics = candidates(&identity, &positions);
    diagnostics.extend(typed_candidates(
        &identity,
        GoRule::Qf1008,
        &[(13, 8), (13, 20)],
        "Base",
    ));
    diagnostics.extend(typed_candidates(
        &identity,
        GoRule::Qf1012,
        &[(14, 2), (15, 2)],
        "",
    ));
    let mut suppressed: Vec<Suppressed> = candidates(&identity, &[(3, 9), (12, 1)])
        .into_iter()
        .map(|diagnostic| Suppressed {
            diagnostic,
            reason: String::new(),
        })
        .collect();
    keep_confirmed(&mut diagnostics, &mut suppressed, &roots, &modules);
    assert_eq!(
        subjects(&diagnostics),
        [
            "3:9 file.Close",
            "6:8 ",
            "8:2 ",
            "9:2 ",
            "13:8 Base",
            "14:2 fmt.Fprintf"
        ]
    );
    assert_eq!(suppressed.len(), 1);
}
