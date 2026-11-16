use regex::Regex;
use std::sync::LazyLock;

const INDENT_CHARS: [char; 2] = [' ', '\t'];
const BULLET_MARKERS: [char; 4] = ['•', '*', '+', '-'];
const NUMBER_TERMINATORS: [char; 2] = ['.', ')'];
const HEADING_MARKER: char = '#';
const OLD_HEADING_FORBIDDEN: &str = ";:!?+*/=[]{}_^°&§~%#@<\">\\";
const POSSESSIVE: &str = "s";
const POSSESSIVE_BEFORE_WORD: &str = "s ";
const LINES_AFTER_OLD_HEADING: usize = 2;

static STARTS_WITH_UPPERCASE_LETTER: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(r"^\p{Lu}").ok());
static ENDS_WITH_LETTER_OR_DIGIT: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"[\p{L}\p{Nd}]$").ok());

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanKind {
    Code,
    Heading,
    List,
    OldHeading,
    Paragraph,
}

#[derive(Debug, Clone, Copy)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub kind: SpanKind,
}

fn line_at<'text>(lines: &[&'text str], index: usize) -> &'text str {
    lines.get(index).copied().unwrap_or_default()
}

pub fn indented(line: &str) -> bool {
    line.starts_with(INDENT_CHARS)
}

fn leading_space(line: &str) -> &str {
    let length = line.len() - line.trim_start_matches(INDENT_CHARS).len();
    line.get(..length).unwrap_or_default()
}

fn common_prefix<'text>(left: &'text str, right: &str) -> &'text str {
    let length = left
        .bytes()
        .zip(right.bytes())
        .take_while(|(left_byte, right_byte)| left_byte == right_byte)
        .count();
    left.get(..length).unwrap_or_default()
}

fn without_empty_edges<'lines, 'text>(lines: &'lines [&'text str]) -> &'lines [&'text str] {
    let start = lines
        .iter()
        .position(|line| !line.is_empty())
        .unwrap_or(lines.len());
    let end = lines
        .iter()
        .rposition(|line| !line.is_empty())
        .map_or(start, |last| last + 1);
    lines.get(start..end).unwrap_or_default()
}

pub fn unindent<'text>(lines: &[&'text str]) -> Vec<&'text str> {
    let trimmed = without_empty_edges(lines);
    let Some((first, rest)) = trimmed.split_first() else {
        return Vec::new();
    };
    let prefix = rest
        .iter()
        .filter(|line| !line.is_empty())
        .fold(leading_space(first), |prefix, line| {
            common_prefix(prefix, leading_space(line))
        });
    let unindented: Vec<&str> = trimmed
        .iter()
        .map(|line| line.strip_prefix(prefix).unwrap_or(line))
        .map(|line| if line.trim().is_empty() { "" } else { line })
        .collect();
    without_empty_edges(&unindented).to_vec()
}

pub fn list_marker(line: &str) -> Option<(&str, &str)> {
    let line = line.trim();
    let (number, rest) = if let Some(rest) = line.strip_prefix(BULLET_MARKERS) {
        ("", rest)
    } else {
        let digits = line.len()
            - line
                .trim_start_matches(|character: char| character.is_ascii_digit())
                .len();
        if digits == 0 {
            return None;
        }
        let (number, after_number) = line.split_at_checked(digits)?;
        (number, after_number.strip_prefix(NUMBER_TERMINATORS)?)
    };
    (indented(rest) && !rest.trim().is_empty()).then_some((number, rest))
}

fn is_list(line: &str) -> bool {
    list_marker(line).is_some()
}

fn is_heading(line: &str) -> bool {
    line.strip_prefix(HEADING_MARKER)
        .is_some_and(|rest| indented(rest) && !rest.trim().is_empty())
}

fn every_match_is_followed_by(line: &str, mark: char, accepts: impl Fn(&str) -> bool) -> bool {
    line.match_indices(mark)
        .all(|(index, found)| accepts(line.get(index + found.len()..).unwrap_or_default()))
}

fn is_old_heading(lines: &[&str], index: usize) -> bool {
    let stands_alone = index > 0
        && line_at(lines, index - 1).is_empty()
        && index + LINES_AFTER_OLD_HEADING < lines.len()
        && line_at(lines, index + 1).is_empty()
        && !indented(line_at(lines, index + LINES_AFTER_OLD_HEADING));
    let line = line_at(lines, index).trim();
    let matches =
        |pattern: &LazyLock<Option<Regex>>| pattern.as_ref().is_some_and(|regex| regex.is_match(line));
    stands_alone
        && matches(&STARTS_WITH_UPPERCASE_LETTER)
        && matches(&ENDS_WITH_LETTER_OR_DIGIT)
        && !line.contains(|character| OLD_HEADING_FORBIDDEN.contains(character))
        && every_match_is_followed_by(line, '\'', |rest| {
            rest == POSSESSIVE || rest.starts_with(POSSESSIVE_BEFORE_WORD)
        })
        && every_match_is_followed_by(line, '.', |rest| !rest.is_empty() && !rest.starts_with(' '))
}

fn indented_span(lines: &[&str], start: usize, force_indent: usize) -> Span {
    let mut unindented_list_allowed = is_list(line_at(lines, start)) && start < force_indent;
    let mut index = start + 1;
    while let Some(line) = lines.get(index) {
        let continues = line.is_empty()
            || index < force_indent
            || indented(line)
            || (unindented_list_allowed && is_list(line));
        if !continues {
            break;
        }
        if line.is_empty() {
            unindented_list_allowed = false;
        }
        index += 1;
    }
    let mut end = index;
    while end > start && line_at(lines, end - 1).is_empty() {
        end -= 1;
    }
    if line_at(lines, end).starts_with('}') {
        end += 1;
    }
    let kind = if is_list(line_at(lines, start)) {
        SpanKind::List
    } else {
        SpanKind::Code
    };
    Span { start, end, kind }
}

// Una línea sin sangría pegada a un bloque sangrado: si es una lista, o termina en `{` o `\`, pasa
// a ser parte del bloque (`force_indent`) en vez del párrafo. Sin párrafo que dejar, no hay tramo.
fn plain_span(lines: &[&str], start: usize, force_indent: &mut usize) -> Option<Span> {
    let stop = start
        + 1
        + lines
            .iter()
            .skip(start + 1)
            .take_while(|line| !line.is_empty() && !indented(line))
            .count();
    let mut end = stop;
    let followed_by_code = lines
        .get(stop)
        .is_some_and(|next| !next.is_empty() && !is_list(next));
    if followed_by_code {
        let previous = line_at(lines, stop - 1);
        if is_list(previous) {
            *force_indent = stop;
            end -= 1;
            while end > start && is_list(line_at(lines, end - 1)) {
                end -= 1;
            }
        } else if previous.ends_with('{') || previous.ends_with('\\') {
            *force_indent = stop;
            end -= 1;
        }
        if start == end && *force_indent > start {
            return None;
        }
    }
    let single_line = end - start == 1;
    let kind = if single_line && is_heading(line_at(lines, start)) {
        SpanKind::Heading
    } else if single_line && is_old_heading(lines, start) {
        SpanKind::OldHeading
    } else {
        SpanKind::Paragraph
    };
    Some(Span { start, end, kind })
}

pub fn parse_spans(lines: &[&str]) -> Vec<Span> {
    let mut spans = Vec::new();
    let mut index = 0;
    let mut force_indent = 0;
    loop {
        index += lines
            .iter()
            .skip(index)
            .take_while(|line| line.is_empty())
            .count();
        if index >= lines.len() {
            return spans;
        }
        let found = if index < force_indent || indented(line_at(lines, index)) {
            Some(indented_span(lines, index, force_indent))
        } else {
            plain_span(lines, index, &mut force_indent)
        };
        if let Some(span) = found {
            index = span.end;
            spans.push(span);
        }
    }
}
