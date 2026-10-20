use std::path::PathBuf;

use super::{Scope, directives, split};
use crate::i18n::Message;
use crate::rules::{Diagnostic, DockerfileRule, MigrationRule, RuleId};

fn diagnostic(rule: RuleId, line: usize) -> Diagnostic {
    Diagnostic {
        path: "Dockerfile".to_owned(),
        line,
        column: 1,
        end_line: line,
        rule,
        message: Message::Rule(rule),
        value: 1,
        limit: 0,
        fingerprint: format!("{}:{line}", rule.as_str()),
        relative_path: "Dockerfile".to_owned(),
        absolute_path: PathBuf::from("/repo/Dockerfile"),
        fix: None,
    }
}

const PINNED: RuleId = RuleId::Dockerfile(DockerfileRule::Dl3008);
const NO_CACHE: RuleId = RuleId::Dockerfile(DockerfileRule::Dl3009);

#[test]
fn reads_codes_reason_and_the_line_each_directive_covers() {
    let source = "FROM debian\n# hadolint ignore=DL3008, DL3009 # la imagen base fija las versiones\n\n# otro comentario\nRUN apt-get install -y curl\n";
    let found = directives(source);
    assert_eq!(found.len(), 1);
    let directive = found.first().expect("hay una directiva");
    assert_eq!(directive.line, 2);
    assert_eq!(directive.codes, ["DL3008", "DL3009"]);
    assert_eq!(directive.reason, "la imagen base fija las versiones");
    assert_eq!(directive.covered_line, Some(5));
    assert_eq!(directive.scope, Scope::NextLine);
}

#[test]
fn respects_the_rules_named_on_the_next_line_only() {
    let source =
        "FROM debian\n# hadolint ignore=DL3008\nRUN apt-get install -y curl\nRUN apt-get install -y git\n";
    let (kept, suppressed) = split(
        source.as_bytes(),
        vec![
            diagnostic(PINNED, 3),
            diagnostic(NO_CACHE, 3),
            diagnostic(PINNED, 4),
        ],
    );
    assert_eq!(suppressed.len(), 1);
    assert_eq!(suppressed.first().map(|item| item.diagnostic.line), Some(3));
    assert_eq!(suppressed.first().map(|item| item.reason.as_str()), Some(""));
    let kept: Vec<(RuleId, usize)> = kept.iter().map(|item| (item.rule, item.line)).collect();
    assert_eq!(kept, [(NO_CACHE, 3), (PINNED, 4)]);
}

#[test]
fn file_directives_cover_every_line() {
    let rule = RuleId::Migration(MigrationRule::BanDropColumn);
    let source = "-- squawk-ignore-file ban-drop-column: la columna ya no se lee\nALTER TABLE a DROP COLUMN b;\nALTER TABLE a DROP COLUMN c;\n";
    let (kept, suppressed) = split(source.as_bytes(), vec![diagnostic(rule, 2), diagnostic(rule, 3)]);
    assert!(kept.is_empty());
    assert_eq!(
        suppressed
            .iter()
            .map(|item| item.reason.as_str())
            .collect::<Vec<_>>(),
        ["la columna ya no se lee", "la columna ya no se lee"]
    );
    let global = directives("# hadolint global ignore=DL3008\nRUN x\n");
    assert_eq!(global.first().map(|directive| directive.scope), Some(Scope::File));
}

#[test]
fn ignores_markers_that_are_not_a_leading_comment() {
    assert_eq!(directives("RUN echo '# hadolint ignore=DL3008'\n"), Vec::new());
    assert_eq!(directives("SELECT 1; -- squawk-ignore\n"), Vec::new());
    let shell = directives("#!/bin/sh\n# shellcheck disable=SC2086\necho $a\n");
    assert_eq!(
        shell.first().map(|directive| directive.covered_line),
        Some(Some(3))
    );
    let (kept, suppressed) = split(b"FROM debian\n", vec![diagnostic(PINNED, 1)]);
    assert_eq!((kept.len(), suppressed.len()), (1, 0));
}
