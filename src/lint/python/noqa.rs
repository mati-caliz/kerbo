use std::collections::HashMap;

use tree_sitter::Node;

use super::Report;
use super::ast::{COMMENT, STRING, text};
use crate::syntax::descendants;

const NOQA: &str = "noqa";
const FILE_PREFIXES: [&str; 2] = ["ruff:", "flake8:"];
const CODE_SEPARATORS: [char; 2] = [',', ' '];
const CONTINUATION: u8 = b'\\';
const LINE_FEED: u8 = b'\n';
const CARRIAGE_RETURN: u8 = b'\r';

#[derive(Debug, Clone, PartialEq, Eq)]
enum Directive {
    All,
    Codes(Vec<String>),
}

impl Directive {
    fn covers(&self, code: &str) -> bool {
        match self {
            Self::All => true,
            Self::Codes(codes) => codes.iter().any(|listed| listed == code),
        }
    }
}

#[derive(Debug, Default)]
pub struct Suppressions {
    lines: HashMap<usize, Directive>,
    file: Option<Directive>,
    spans: Vec<(usize, usize)>,
}

impl Suppressions {
    pub fn new(root: Node<'_>, source: &[u8]) -> Self {
        let mut suppressions = Self::default();
        let nodes = descendants(root);
        for comment in nodes.iter().filter(|node| node.kind() == COMMENT) {
            suppressions.read_comment(*comment, source);
        }
        let strings = nodes
            .iter()
            .filter(|node| node.kind() == STRING && node.start_position().row != node.end_position().row)
            .map(|string| (string.start_position().row, string.end_position().row));
        let mut spans: Vec<(usize, usize)> = strings.chain(continuations(root, source)).collect();
        spans.sort_unstable();
        suppressions.spans = merged(spans);
        suppressions
    }

    pub fn silences(&self, report: &Report) -> bool {
        let code = report.rule.as_str().to_uppercase();
        if self
            .file
            .as_ref()
            .is_some_and(|directive| directive.covers(&code))
        {
            return true;
        }
        std::iter::once(report.start.row)
            .chain(report.statement_row)
            .map(|row| self.mapped(row))
            .any(|row| {
                self.lines
                    .get(&row)
                    .is_some_and(|directive| directive.covers(&code))
            })
    }

    fn mapped(&self, row: usize) -> usize {
        self.spans
            .iter()
            .find(|(first, last)| (*first..=*last).contains(&row))
            .map_or(row, |(_, last)| *last)
    }

    fn read_comment(&mut self, comment: Node<'_>, source: &[u8]) {
        let content = text(comment, source);
        if let Some(directive) = file_directive(content) {
            self.file = Some(directive);
        } else if let Some(directive) = line_directive(content) {
            self.lines.insert(comment.start_position().row, directive);
        }
    }
}

fn file_directive(comment: &str) -> Option<Directive> {
    let body = comment.trim_start_matches('#').trim_start();
    let lowered = body.to_lowercase();
    let prefix = FILE_PREFIXES.iter().find(|prefix| lowered.starts_with(*prefix))?;
    let rest = body.get(prefix.len()..)?.trim_start();
    let after = rest
        .get(..NOQA.len())
        .filter(|word| word.eq_ignore_ascii_case(NOQA))
        .and_then(|_| rest.get(NOQA.len()..))?;
    directive_after(after)
}

fn line_directive(comment: &str) -> Option<Directive> {
    comment.match_indices('#').find_map(|(index, _)| {
        let rest = comment.get(index.saturating_add(1)..)?.trim_start();
        let word = rest.get(..NOQA.len())?;
        word.eq_ignore_ascii_case(NOQA)
            .then(|| rest.get(NOQA.len()..).and_then(directive_after))
            .flatten()
    })
}

fn directive_after(rest: &str) -> Option<Directive> {
    let Some(listed) = rest.trim_start().strip_prefix(':') else {
        return Some(Directive::All);
    };
    let codes: Vec<String> = listed
        .split(CODE_SEPARATORS)
        .map(str::trim)
        .skip_while(|code| code.is_empty())
        .take_while(|code| code.is_empty() || is_code(code))
        .filter(|code| !code.is_empty())
        .map(str::to_owned)
        .collect();
    (!codes.is_empty()).then_some(Directive::Codes(codes))
}

fn is_code(word: &str) -> bool {
    let digits = word.trim_start_matches(|letter: char| letter.is_ascii_uppercase());
    digits.len() < word.len() && !digits.is_empty() && digits.chars().all(|digit| digit.is_ascii_digit())
}

fn continuations(root: Node<'_>, source: &[u8]) -> Vec<(usize, usize)> {
    let mut spans: Vec<(usize, usize)> = Vec::new();
    let mut offset: usize = 0;
    for (row, line) in source.split(|byte| *byte == LINE_FEED).enumerate() {
        let content = line.strip_suffix(&[CARRIAGE_RETURN]).unwrap_or(line);
        let backslash = offset.saturating_add(content.len()).saturating_sub(1);
        offset = offset.saturating_add(line.len()).saturating_add(1);
        if content.last() != Some(&CONTINUATION) || inside_token(root, backslash) {
            continue;
        }
        match spans.last_mut() {
            Some((_, last)) if *last == row => *last = row.saturating_add(1),
            _ => spans.push((row, row.saturating_add(1))),
        }
    }
    spans
}

fn inside_token(root: Node<'_>, byte: usize) -> bool {
    root.descendant_for_byte_range(byte, byte.saturating_add(1))
        .is_some_and(|node| {
            std::iter::successors(Some(node), Node::parent)
                .any(|ancestor| matches!(ancestor.kind(), STRING | COMMENT))
        })
}

fn merged(spans: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (first, last) in spans {
        match merged.last_mut() {
            Some((_, previous_last)) if first <= *previous_last => {
                *previous_last = (*previous_last).max(last);
            }
            _ => merged.push((first, last)),
        }
    }
    merged
}
