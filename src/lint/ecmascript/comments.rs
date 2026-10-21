use std::sync::LazyLock;

use regex::{Captures, Regex};
use tree_sitter::Node;

use super::nodes::{COMMENT, PROGRAM};
use super::{Context, report};
use crate::lint::LintFinding;
use crate::rules::EslintRule;

const LINE_PREFIX: &str = "//";
const BLOCK_PREFIX: &str = "/*";
const BLOCK_SUFFIX: &str = "*/";
const LINE_BREAKS: &[char] = &['\n', '\r', '\u{2028}', '\u{2029}'];
const NOCHECK: &str = "nocheck";
const CHECK: &str = "check";
const IGNORE: &str = "ignore";
const EXPECT_ERROR: &str = "expect-error";
const DIRECTIVE_PREFIX: &str = "@ts-";
const MINIMUM_DESCRIPTION_LENGTH: usize = 3;
const HASH_BANG: &str = "hash_bang_line";
const DIRECTIVE_GROUP: &str = "directive";
const DESCRIPTION_GROUP: &str = "description";

static PRAGMA: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(
        r"^///?[\t\n\x0B\x0C\r \u{a0}\u{1680}\u{2000}-\u{200a}\u{2028}\u{2029}\u{202f}\u{205f}\u{3000}\u{feff}]*@ts-(?<directive>check|nocheck)(?<description>[^\n\r\u{2028}\u{2029}]*)$",
    )
    .ok()
});
static SINGLE_LINE: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(
        r"^/*[\t\n\x0B\x0C\r \u{a0}\u{1680}\u{2000}-\u{200a}\u{2028}\u{2029}\u{202f}\u{205f}\u{3000}\u{feff}]*@ts-(?<directive>expect-error|ignore)(?<description>[^\n\r\u{2028}\u{2029}]*)",
    )
    .ok()
});
static MULTI_LINE: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(
        r"^[\t\n\x0B\x0C\r \u{a0}\u{1680}\u{2000}-\u{200a}\u{2028}\u{2029}\u{202f}\u{205f}\u{3000}\u{feff}]*[/*]*[\t\n\x0B\x0C\r \u{a0}\u{1680}\u{2000}-\u{200a}\u{2028}\u{2029}\u{202f}\u{205f}\u{3000}\u{feff}]*@ts-(?<directive>expect-error|ignore)(?<description>[^\n\r\u{2028}\u{2029}]*)",
    )
    .ok()
});

struct Directive {
    name: String,
    description: String,
}

pub fn ts_comments(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let first_statement_row = context
        .nodes
        .first()
        .filter(|root| root.kind() == PROGRAM)
        .and_then(|root| first_statement(*root))
        .map(|statement| statement.start_position().row);
    context
        .of_kind(&[COMMENT])
        .filter(|comment| context.text(*comment).contains(DIRECTIVE_PREFIX))
        .filter_map(|comment| directive(context.text(comment)).map(|found| (comment, found)))
        .filter(|(comment, found)| {
            found.name != NOCHECK || first_statement_row.is_none_or(|row| row > comment.start_position().row)
        })
        .filter(|(_, found)| is_banned(found))
        .map(|(comment, found)| {
            report(
                EslintRule::BanTsComment,
                comment,
                &format!("{DIRECTIVE_PREFIX}{}", found.name),
            )
        })
        .collect()
}

fn first_statement(root: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = root.walk();
    root.named_children(&mut cursor)
        .find(|child| ![COMMENT, HASH_BANG].contains(&child.kind()))
}

fn is_banned(found: &Directive) -> bool {
    match found.name.as_str() {
        IGNORE | NOCHECK => true,
        EXPECT_ERROR => found.description.trim().chars().count() < MINIMUM_DESCRIPTION_LENGTH,
        _ => false,
    }
}

fn directive(comment: &str) -> Option<Directive> {
    if let Some(value) = comment.strip_prefix(LINE_PREFIX) {
        return matched(PRAGMA.as_ref(), comment)
            .filter(|found| [CHECK, NOCHECK].contains(&found.name.as_str()))
            .or_else(|| matched(SINGLE_LINE.as_ref(), value));
    }
    let value = comment.strip_prefix(BLOCK_PREFIX)?;
    let value = value.strip_suffix(BLOCK_SUFFIX).unwrap_or(value);
    let last_line = value.rsplit(LINE_BREAKS).next().unwrap_or(value);
    matched(MULTI_LINE.as_ref(), last_line)
}

fn matched(pattern: Option<&Regex>, text: &str) -> Option<Directive> {
    pattern?.captures(text).map(|captures| Directive {
        name: group(&captures, DIRECTIVE_GROUP),
        description: group(&captures, DESCRIPTION_GROUP),
    })
}

fn group(captures: &Captures<'_>, name: &str) -> String {
    captures
        .name(name)
        .map_or_else(String::new, |found| found.as_str().to_owned())
}
