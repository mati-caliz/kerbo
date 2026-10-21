use tree_sitter::Node;

use super::Report;
use super::ast::{COMMENT, STRING};
use crate::fix::Fix;
use crate::rules::PythonRule;

const OPENING: [&str; 3] = ["(", "[", "{"];
const CLOSING: [&str; 3] = [")", "]", "}"];
const COMPOUND_KEYWORDS: [&str; 12] = [
    "case", "class", "elif", "else", "except", "finally", "for", "if", "match", "try", "while", "with",
];
const LAMBDA: &str = "lambda";
const COLON: &str = ":";
const SEMICOLON: &str = ";";
const ELLIPSIS: &str = "ellipsis";
const LINE_FEED: u8 = b'\n';
const CARRIAGE_RETURN: u8 = b'\r';
const CONTINUATION: u8 = b'\\';

#[derive(Debug, Default)]
struct LogicalLine<'tree> {
    colon: Option<Node<'tree>>,
    semicolon: Option<Node<'tree>>,
    has_keyword: bool,
    allows_ellipsis: bool,
}

pub fn reports(root: Node<'_>, source: &[u8]) -> Vec<Report> {
    let mut tokens = Vec::new();
    collect_tokens(root, &mut tokens);
    let mut reports = Vec::new();
    let mut line = LogicalLine::default();
    let mut depth: usize = 0;
    let mut previous_end = 0;
    for token in tokens {
        let gap = source.get(previous_end..token.start_byte()).unwrap_or_default();
        previous_end = token.end_byte();
        if depth == 0 && has_newline(gap) {
            reports.extend(line.end(source));
            line = LogicalLine::default();
        }
        if token.kind() == COMMENT {
            continue;
        }
        if token.kind() == ELLIPSIS && line.allows_ellipsis {
            line.allows_ellipsis = false;
            continue;
        }
        depth = nesting(depth, token.kind());
        if depth > 0 {
            continue;
        }
        reports.extend(line.accept(token));
    }
    reports.extend(line.end(source));
    reports
}

impl<'tree> LogicalLine<'tree> {
    fn end(&self, source: &[u8]) -> Option<Report> {
        self.semicolon.map(|semicolon| {
            let fix = Fix::replace(semicolon, "", source);
            Report::at(PythonRule::E703, semicolon, "").with_fix(Some(fix))
        })
    }

    fn accept(&mut self, token: Node<'tree>) -> Vec<Report> {
        let mut reports = Vec::new();
        match token.kind() {
            COLON => {
                if self.has_keyword {
                    self.colon = Some(token);
                    self.allows_ellipsis = true;
                }
            }
            SEMICOLON => {
                self.semicolon = Some(token);
                self.allows_ellipsis = false;
            }
            _ => reports.extend(self.statement_follows()),
        }
        if token.kind() == LAMBDA {
            self.reset_keywords();
        } else if COMPOUND_KEYWORDS.contains(&token.kind()) {
            self.has_keyword = true;
        }
        reports
    }

    fn statement_follows(&mut self) -> Vec<Report> {
        let mut reports = Vec::new();
        if let Some(semicolon) = self.semicolon.take() {
            reports.push(Report::at(PythonRule::E702, semicolon, ""));
            self.allows_ellipsis = false;
        }
        if let Some(colon) = self.colon {
            reports.push(Report::at(PythonRule::E701, colon, ""));
            self.reset_keywords();
            self.allows_ellipsis = false;
        }
        reports
    }

    const fn reset_keywords(&mut self) {
        self.colon = None;
        self.has_keyword = false;
    }
}

fn nesting(depth: usize, kind: &str) -> usize {
    if OPENING.contains(&kind) {
        depth.saturating_add(1)
    } else if CLOSING.contains(&kind) {
        depth.saturating_sub(1)
    } else {
        depth
    }
}

fn collect_tokens<'tree>(node: Node<'tree>, tokens: &mut Vec<Node<'tree>>) {
    if node.child_count() == 0 || node.kind() == STRING {
        if node.end_byte() > node.start_byte() {
            tokens.push(node);
        }
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_tokens(child, tokens);
    }
}

fn has_newline(gap: &[u8]) -> bool {
    gap.iter().enumerate().any(|(index, byte)| {
        *byte == LINE_FEED && {
            let before = gap.get(..index).unwrap_or_default();
            let before = before.strip_suffix(&[CARRIAGE_RETURN]).unwrap_or(before);
            before.last() != Some(&CONTINUATION)
        }
    })
}
