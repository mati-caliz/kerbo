use tree_sitter::{Node, Point};

use super::java::{ANNOTATION, NAME_FIELD, modifiers, qualified_name, text};
use super::{LintFinding, LintKind};
use crate::syntax::{descendants, named_children};

const SUPPRESS_WARNINGS: &str = "SuppressWarnings";
const ARGUMENTS_FIELD: &str = "arguments";
const STRING_LITERAL: &str = "string_literal";
const STRING_FRAGMENT: &str = "string_fragment";
const ALL: &str = "all";
const PMD: &str = "PMD";
const RULE_SEPARATOR: &str = ".";
const LINE_COMMENT: &str = "//";
const NOPMD: &str = "NOPMD";

pub fn drop_suppressed(findings: &mut Vec<LintFinding>, root: Node<'_>, source: &[u8]) {
    if !findings.iter().any(|finding| pmd_rule(finding.kind).is_some()) {
        return;
    }
    let suppressing = suppressing_declarations(root, source);
    let lines: Vec<&[u8]> = source.split(|&byte| byte == b'\n').collect();
    findings.retain(|finding| {
        let Some(rule) = pmd_rule(finding.kind) else {
            return true;
        };
        let position = Point::new(finding.line.saturating_sub(1), finding.column.saturating_sub(1));
        let annotated = suppressing.iter().any(|(declaration, names)| {
            declaration.start_position() <= position
                && position < declaration.end_position()
                && names.iter().any(|name| silences(name, rule))
        });
        let marked = lines
            .get(position.row)
            .is_some_and(|line| has_nopmd_comment(line));
        !annotated && !marked
    });
}

const PMD_RULES: [(LintKind, &str); 2] = [
    (LintKind::RedundantModifier, "UnnecessaryModifier"),
    (
        LintKind::FinalClass,
        "ClassWithOnlyPrivateConstructorsShouldBeFinal",
    ),
];

fn pmd_rule(kind: LintKind) -> Option<&'static str> {
    PMD_RULES
        .iter()
        .find(|(lint, _)| *lint == kind)
        .map(|&(_, name)| name)
}

fn silences(name: &str, rule: &str) -> bool {
    name == ALL
        || name == PMD
        || name
            .strip_prefix(PMD)
            .and_then(|rest| rest.strip_prefix(RULE_SEPARATOR))
            .is_some_and(|named| named == rule)
}

fn has_nopmd_comment(line: &[u8]) -> bool {
    let line = String::from_utf8_lossy(line);
    line.find(LINE_COMMENT)
        .and_then(|start| line.get(start..))
        .is_some_and(|comment| comment.contains(NOPMD))
}

fn suppressing_declarations<'tree>(root: Node<'tree>, source: &[u8]) -> Vec<(Node<'tree>, Vec<String>)> {
    descendants(root)
        .into_iter()
        .filter_map(|declaration| {
            let names: Vec<String> = modifiers(declaration)
                .map(named_children)
                .unwrap_or_default()
                .into_iter()
                .filter(|annotation| {
                    annotation.kind() == ANNOTATION && is_suppress_warnings(*annotation, source)
                })
                .flat_map(|annotation| suppressed_names(annotation, source))
                .collect();
            (!names.is_empty()).then_some((declaration, names))
        })
        .collect()
}

fn is_suppress_warnings(annotation: Node<'_>, source: &[u8]) -> bool {
    annotation
        .child_by_field_name(NAME_FIELD)
        .is_some_and(|name| qualified_name(name, source).ends_with(SUPPRESS_WARNINGS))
}

fn suppressed_names(annotation: Node<'_>, source: &[u8]) -> Vec<String> {
    annotation
        .child_by_field_name(ARGUMENTS_FIELD)
        .map(descendants)
        .unwrap_or_default()
        .into_iter()
        .filter(|node| node.kind() == STRING_LITERAL)
        .map(|literal| {
            named_children(literal)
                .into_iter()
                .filter(|part| part.kind() == STRING_FRAGMENT)
                .map(|part| text(part, source))
                .collect()
        })
        .collect()
}
