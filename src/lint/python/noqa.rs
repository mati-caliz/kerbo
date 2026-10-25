mod ruff_codes;
mod unused;

use std::collections::HashMap;

use tree_sitter::{Node, Point};

use super::Report;
use super::ast::{COMMENT, STRING, text};
use crate::rules::PythonRule;
use crate::syntax::descendants;
use ruff_codes::REDIRECTS;
pub use unused::{Check, unused_directives};

const NOQA: &str = "noqa";
const FLAKE8: &str = "flake8";
const FILE_PREFIXES: [&str; 2] = ["ruff:", "flake8:"];
const CODE_SEPARATORS: [char; 2] = [',', ' '];
const COMMENT_MARKER: char = '#';
const CODES_MARKER: char = ':';
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
            Self::Codes(codes) => codes.iter().any(|listed| redirected(listed) == code),
        }
    }
}

// `get_redirect_target` de ruff: un código viejo apunta al de la regla que lo reemplazó.
fn redirected(code: &str) -> &str {
    REDIRECTS
        .binary_search_by(|(old, _)| (*old).cmp(code))
        .ok()
        .and_then(|index| REDIRECTS.get(index))
        .map_or(code, |(_, new)| *new)
}

#[derive(Debug, Clone)]
struct Placed {
    directive: Directive,
    start: Point,
    start_byte: usize,
    end_byte: usize,
    file: bool,
    flake8: bool,
}

#[derive(Debug, Default)]
pub struct Suppressions {
    lines: HashMap<usize, Placed>,
    files: Vec<Placed>,
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
        self.exempts(&code) || self.silencing_row(report, &code).is_some()
    }

    // La exención de archivo (`# ruff: noqa`), toda o por códigos.
    fn exempts(&self, code: &str) -> bool {
        self.files.iter().any(|file| file.directive.covers(code))
    }

    // La línea cuyo `noqa` silencia el aviso: la de la sentencia y después la del aviso, como el
    // `parent` de ruff.
    fn silencing_row(&self, report: &Report, code: &str) -> Option<usize> {
        report
            .statement_row
            .into_iter()
            .chain(std::iter::once(report.start.row))
            .map(|row| self.mapped(row))
            .find(|row| {
                self.lines
                    .get(row)
                    .is_some_and(|placed| placed.directive.covers(code))
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
        let start = comment.start_position();
        let comment_start = comment.start_byte();
        if let Some((directive, end)) = file_directive(content) {
            self.files.push(Placed {
                directive,
                start,
                start_byte: comment_start,
                end_byte: comment_start.saturating_add(end),
                file: true,
                flake8: content.contains(FLAKE8),
            });
        } else if let Some((offset, directive, end)) = line_directive(content) {
            let start = Point {
                row: start.row,
                column: start.column.saturating_add(offset),
            };
            let placed = Placed {
                directive,
                start,
                start_byte: comment_start.saturating_add(offset),
                end_byte: comment_start.saturating_add(end),
                file: false,
                flake8: false,
            };
            self.lines.insert(start.row, placed);
        }
    }
}

// La directiva de archivo y dónde termina adentro del comentario.
fn file_directive(comment: &str) -> Option<(Directive, usize)> {
    let body = comment.trim_start_matches(COMMENT_MARKER).trim_start();
    let lowered = body.to_lowercase();
    let prefix = FILE_PREFIXES.iter().find(|prefix| lowered.starts_with(*prefix))?;
    let rest = body.get(prefix.len()..)?.trim_start();
    let after = rest
        .get(..NOQA.len())
        .filter(|word| word.eq_ignore_ascii_case(NOQA))
        .and_then(|_| rest.get(NOQA.len()..))?;
    let (directive, consumed) = directive_after(after)?;
    let end = comment.len().saturating_sub(after.len()).saturating_add(consumed);
    Some((directive, end))
}

// La directiva, dónde empieza su `#` y dónde termina adentro del comentario.
fn line_directive(comment: &str) -> Option<(usize, Directive, usize)> {
    comment.match_indices(COMMENT_MARKER).find_map(|(index, _)| {
        let rest = comment.get(index.saturating_add(1)..)?.trim_start();
        let word = rest.get(..NOQA.len())?;
        let after = word
            .eq_ignore_ascii_case(NOQA)
            .then(|| rest.get(NOQA.len()..))
            .flatten()?;
        let (directive, consumed) = directive_after(after)?;
        let end = comment.len().saturating_sub(after.len()).saturating_add(consumed);
        Some((index, directive, end))
    })
}

// Lo que sigue a `noqa`, como el `lex_directive` de ruff: nada, `#` o espacios sin `:` es un `noqa`
// general; `:` abre la lista de códigos; cualquier otra cosa (`noqa[E501]`) no es una directiva.
// También cuánto de `rest` ocupa la directiva: nada para un `noqa` general, hasta el último código si
// tiene lista.
fn directive_after(rest: &str) -> Option<(Directive, usize)> {
    let starts_valid = rest
        .chars()
        .next()
        .is_none_or(|first| first == COMMENT_MARKER || first == CODES_MARKER || first.is_whitespace());
    if !starts_valid {
        return None;
    }
    let Some(listed) = rest.trim_start().strip_prefix(CODES_MARKER) else {
        return Some((Directive::All, 0));
    };
    let found = listed_codes(listed);
    let listed_start = rest.len().saturating_sub(listed.len());
    let consumed = found
        .last()
        .map(|(position, code)| listed_start.saturating_add(*position).saturating_add(code.len()))?;
    let codes: Vec<String> = found.into_iter().map(|(_, code)| code.to_owned()).collect();
    Some((Directive::Codes(codes), consumed))
}

// Los códigos de la lista después de `noqa:`, con dónde empieza cada uno adentro de `listed`. La lista
// termina en la primera palabra que no es un código.
fn listed_codes(listed: &str) -> Vec<(usize, &str)> {
    let mut words = Vec::new();
    let mut start = None;
    for (index, letter) in listed.char_indices().chain(std::iter::once((listed.len(), ' '))) {
        let separator = CODE_SEPARATORS.contains(&letter) || letter.is_whitespace();
        match (separator, start) {
            (true, Some(first)) => {
                words.extend(listed.get(first..index).map(|word| (first, word)));
                start = None;
            }
            (false, None) => start = Some(index),
            _ => {}
        }
    }
    words.into_iter().take_while(|(_, word)| is_code(word)).collect()
}

// RUF101: un código viejo en un `noqa` que ruff redirige al de otra regla. Avisa en el código.
pub fn redirected_codes(root: Node<'_>, source: &[u8]) -> Vec<Report> {
    let mut reports = Vec::new();
    for comment in descendants(root)
        .into_iter()
        .filter(|node| node.kind() == COMMENT)
    {
        let content = text(comment, source);
        let Some(listed) = codes_suffix(content) else {
            continue;
        };
        let offset = content.len().saturating_sub(listed.len());
        let start = comment.start_position();
        for (position, code) in listed_codes(listed) {
            if redirected(code) != code {
                let at = Point {
                    row: start.row,
                    column: start.column.saturating_add(offset).saturating_add(position),
                };
                reports.push(Report::at_point(PythonRule::RUF101, at, code));
            }
        }
    }
    reports
}

// Lo que sigue al `:` de un `# noqa:` o un `# ruff: noqa:`, como sufijo del comentario.
fn codes_suffix(comment: &str) -> Option<&str> {
    let body = comment.trim_start_matches(COMMENT_MARKER).trim_start();
    let lowered = body.to_lowercase();
    let after = if let Some(prefix) = FILE_PREFIXES.iter().find(|prefix| lowered.starts_with(*prefix)) {
        let rest = body.get(prefix.len()..)?.trim_start();
        rest.get(..NOQA.len())
            .filter(|word| word.eq_ignore_ascii_case(NOQA))
            .and_then(|_| rest.get(NOQA.len()..))?
    } else {
        comment.match_indices(COMMENT_MARKER).find_map(|(index, _)| {
            let rest = comment.get(index.saturating_add(1)..)?.trim_start();
            rest.get(..NOQA.len())
                .filter(|word| word.eq_ignore_ascii_case(NOQA))
                .and_then(|_| rest.get(NOQA.len()..))
                .filter(|after| directive_after(after).is_some())
        })?
    };
    after.trim_start().strip_prefix(CODES_MARKER)
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
