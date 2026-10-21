use std::collections::HashMap;
use std::ops::Range;

use tree_sitter::Node;

use super::syntax::{CASE_ITEM, COMMENT, ELIF_CLAUSE, ELSE_CLAUSE, PROGRAM, children, text};
use crate::syntax::one_based;

const DIRECTIVE_PREFIX: &str = "shellcheck";
const COMMENT_START: char = '#';

const SHELL_KEY: &str = "shell";
const KEY_VALUE_SEPARATOR: char = '=';
const RANGE_SEPARATOR: char = '-';
const CODE_PREFIX: &str = "sc";
const ALL_CODES: &str = "all";
const ALL_CODES_RANGE: Range<u32> = 0..1_000_000;
const QUOTES: &[char] = &['\'', '"'];
const UNANNOTATABLE: &[&str] = &[ELIF_CLAUSE, ELSE_CLAUSE, CASE_ITEM];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Directive {
    pub shell: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Coverage {
    File,
    Lines(usize, usize),
}

#[derive(Debug, Default)]
pub struct Directives {
    pub shell: Option<String>,
    pub coverage: HashMap<usize, Coverage>,
}

impl Directives {
    pub fn read(root: Node<'_>, source: &[u8]) -> Self {
        let mut directives = Self::default();
        let mut pending = vec![root];
        while let Some(node) = pending.pop() {
            directives.annotate_children(node, source);
            pending.extend(children(node).into_iter().filter(|child| child.child_count() > 0));
        }
        directives
    }

    fn annotate_children(&mut self, node: Node<'_>, source: &[u8]) {
        let mut waiting: Vec<(Node<'_>, Directive)> = Vec::new();
        let mut previous_end_row = None;
        let mut before_first_statement = node.kind() == PROGRAM;
        for child in children(node) {
            if child.kind() == COMMENT {
                let trailing = previous_end_row == Some(child.start_position().row);
                waiting.extend(
                    parse(text(child, source))
                        .filter(|_| !trailing)
                        .map(|directive| (child, directive)),
                );
                continue;
            }
            previous_end_row = Some(child.end_position().row);
            let found = std::mem::take(&mut waiting);
            if before_first_statement && child.is_named() {
                before_first_statement = false;
                self.cover_file(found);
            } else if child.is_named() && !UNANNOTATABLE.contains(&child.kind()) {
                self.cover_statement(child, found);
            }
        }
        if before_first_statement {
            self.cover_file(waiting);
        }
    }

    fn cover_statement(&mut self, statement: Node<'_>, found: Vec<(Node<'_>, Directive)>) {
        let lines = Coverage::Lines(
            one_based(statement.start_position().row),
            one_based(statement.end_position().row),
        );
        for (comment, _) in found {
            self.coverage
                .insert(one_based(comment.start_position().row), lines);
        }
    }

    fn cover_file(&mut self, found: Vec<(Node<'_>, Directive)>) {
        if let Some(shell) = found
            .iter()
            .rev()
            .find_map(|(_, directive)| directive.shell.clone())
        {
            self.shell = Some(shell);
        }
        for (comment, _) in found {
            self.coverage
                .insert(one_based(comment.start_position().row), Coverage::File);
        }
    }
}

pub fn parse(comment: &str) -> Option<Directive> {
    let body = comment.strip_prefix(COMMENT_START)?.trim_start();
    let pairs = body.strip_prefix(DIRECTIVE_PREFIX)?;
    if !pairs.starts_with(char::is_whitespace) {
        return None;
    }
    let mut directive = Directive::default();
    for pair in pairs.split_whitespace() {
        if pair.starts_with(COMMENT_START) {
            break;
        }
        let Some((key, value)) = pair.split_once(KEY_VALUE_SEPARATOR) else {
            break;
        };
        if key == SHELL_KEY {
            directive.shell = Some(value.trim_matches(QUOTES).to_owned());
        }
    }
    Some(directive)
}

pub fn disables(element: &str, rule: &str) -> bool {
    let Some(rule_code) = code(rule) else {
        return false;
    };
    let element = element.to_ascii_lowercase();
    let range = if element == ALL_CODES {
        Some(ALL_CODES_RANGE)
    } else {
        match element.split_once(RANGE_SEPARATOR) {
            Some((from, to)) => code(from).zip(code(to)).map(|(from, to)| from..to),
            None => code(&element).map(|single| single..single.saturating_add(1)),
        }
    };
    range.is_some_and(|range| range.contains(&rule_code))
}

fn code(text: &str) -> Option<u32> {
    let lowered = text.to_ascii_lowercase();
    lowered.strip_prefix(CODE_PREFIX).unwrap_or(&lowered).parse().ok()
}
