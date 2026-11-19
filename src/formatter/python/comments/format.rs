use std::borrow::Cow;

use super::{LinePosition, SourceComment};
use crate::formatter::document::{LineMode, TagKind};
use crate::formatter::python::ast::NodeId;
use crate::formatter::python::context::{Formatter, NodeLevel};
use crate::formatter::python::trivia::{lines_after, lines_after_ignoring_trivia, lines_before};

const COMMENT_START: char = '#';
const EMPTY_COMMENT: &str = "#";
const NO_BREAK_SPACE: char = '\u{A0}';
const KEPT_AFTER_THE_HASH: [char; 6] = [' ', '!', ':', '#', '\'', '|'];
const TYPE_PRAGMA: &str = "type:";
const SPACES_BEFORE_A_TRAILING_COMMENT: u32 = 2;
const PRAGMAS_WITH_COLON: [&str; 8] = [
    "isort", "type", "pyright", "pyrefly", "pylint", "flake8", "ruff", "ty",
];
const NOQA: &str = "noqa";
const NOSEC: &str = "nosec";
const TOP_LEVEL_EMPTY_LINES: usize = 2;
const NESTED_EMPTY_LINES: usize = 1;

fn unformatted(comments: &[SourceComment]) -> impl Iterator<Item = &SourceComment> {
    comments.iter().filter(|comment| comment.is_unformatted())
}

pub fn leading_comments(f: &mut Formatter<'_>, comments: &[SourceComment]) {
    for comment in unformatted(comments) {
        let lines_after_comment = lines_after(f.source, comment.span.end);
        format_comment(f, comment);
        empty_lines(f, lines_after_comment);
        comment.mark_formatted();
    }
}

pub fn leading_node_comments(f: &mut Formatter<'_>, node: NodeId) {
    let comments = f.comments;
    leading_comments(f, comments.leading(node));
}

// Los comentarios antes de un `else`, `elif`, `except` o `finally`: conservan las líneas en
// blanco que tenían arriba.
pub fn leading_alternate_branch_comments(
    f: &mut Formatter<'_>,
    comments: &[SourceComment],
    last_node: Option<NodeId>,
) {
    if let Some(first_leading) = comments.first() {
        let lines = lines_before(f.source, first_leading.span.start);
        empty_lines(f, lines);
        leading_comments(f, comments);
    } else if let Some(last_preceding) = last_node {
        let lines = lines_after_ignoring_trivia(f.source, f.ast.end(last_preceding));
        empty_lines(f, lines);
    }
}

pub fn trailing_comments(f: &mut Formatter<'_>, comments: &[SourceComment]) {
    let mut has_trailing_own_line_comment = false;
    for trailing in unformatted(comments) {
        has_trailing_own_line_comment |= trailing.is_own_line();
        if has_trailing_own_line_comment {
            let lines_before_comment = lines_before(f.source, trailing.span.start);
            f.start_line_suffix(0);
            empty_lines(f, lines_before_comment);
            format_comment(f, trailing);
            f.end(TagKind::LineSuffix);
            f.expand_parent();
        } else {
            trailing_end_of_line_comment(f, trailing);
        }
        trailing.mark_formatted();
    }
}

pub fn dangling_comments(f: &mut Formatter<'_>, comments: &[SourceComment]) {
    let mut first = true;
    for comment in unformatted(comments) {
        if first {
            match comment.line_position {
                LinePosition::OwnLine => f.line(LineMode::Hard),
                LinePosition::EndOfLine => {
                    f.space();
                    f.space();
                }
            }
        }
        let lines_after_comment = lines_after(f.source, comment.span.end);
        format_comment(f, comment);
        empty_lines(f, lines_after_comment);
        comment.mark_formatted();
        first = false;
    }
}

pub fn dangling_node_comments(f: &mut Formatter<'_>, node: NodeId) {
    let comments = f.comments;
    dangling_comments(f, comments.dangling(node));
}

// Los comentarios que quedaron después de un paréntesis que abre, al final de esa línea.
pub fn dangling_open_parenthesis_comments(f: &mut Formatter<'_>, comments: &[SourceComment]) {
    for comment in unformatted(comments) {
        trailing_end_of_line_comment(f, comment);
        comment.mark_formatted();
    }
}

pub fn format_comment(f: &mut Formatter<'_>, comment: &SourceComment) {
    let normalized = normalize_comment(f.text_of(comment.span));
    f.text(&normalized);
}

pub fn empty_lines(f: &mut Formatter<'_>, lines: usize) {
    match f.node_level {
        NodeLevel::TopLevel(_) => match lines {
            0 | 1 => f.line(LineMode::Hard),
            2 => f.line(LineMode::Empty),
            _ => {
                f.line(LineMode::Empty);
                f.line(LineMode::Empty);
            }
        },
        NodeLevel::CompoundStatement => {
            if lines <= 1 {
                f.line(LineMode::Hard);
            } else {
                f.line(LineMode::Empty);
            }
        }
        NodeLevel::Expression(_) | NodeLevel::ParenthesizedExpression => f.line(LineMode::Hard),
    }
}

// Un comentario al final de la línea no cuenta para el ancho, salvo la parte que no es un pragma
// (`# noqa`, `# type:`): así un pragma nunca parte la línea que anota.
fn trailing_end_of_line_comment(f: &mut Formatter<'_>, comment: &SourceComment) {
    let normalized = normalize_comment(f.text_of(comment.span));
    let non_pragma_part = match find_trailing_pragma_offset(&normalized) {
        Some(offset) => normalized.get(..offset).unwrap_or_default().trim_end(),
        None => &normalized,
    };
    let reserved_width = if non_pragma_part.is_empty() {
        0
    } else {
        SPACES_BEFORE_A_TRAILING_COMMENT.saturating_add(width_of(non_pragma_part))
    };
    f.start_line_suffix(reserved_width);
    f.space();
    f.space();
    f.text(&normalized);
    f.end(TagKind::LineSuffix);
    f.expand_parent();
}

fn width_of(text: &str) -> u32 {
    crate::formatter::document::text_width(text).unwrap_or(0)
}

fn is_pragma_comment(comment: &str) -> bool {
    let Some(content) = comment.strip_prefix(COMMENT_START) else {
        return false;
    };
    let trimmed = content.trim_start();
    let starts_with_noqa = trimmed
        .get(..NOQA.len())
        .is_some_and(|start| start.eq_ignore_ascii_case(NOQA));
    if starts_with_noqa || trimmed.starts_with(NOSEC) {
        return true;
    }
    trimmed
        .split_once(':')
        .is_some_and(|(maybe_pragma, _)| PRAGMAS_WITH_COLON.contains(&maybe_pragma))
}

fn find_trailing_pragma_offset(comment: &str) -> Option<usize> {
    comment
        .match_indices(COMMENT_START)
        .map(|(offset, _)| offset)
        .find(|offset| is_pragma_comment(comment.get(*offset..).unwrap_or_default()))
}

// `#comentario` pasa a `# comentario`; un `#!`, `#:` o `#'` queda como está.
pub fn normalize_comment(comment_text: &str) -> Cow<'_, str> {
    let trimmed = comment_text.trim_end();
    let content = trimmed.strip_prefix(COMMENT_START).unwrap_or(trimmed);
    if content.is_empty() {
        return Cow::Borrowed(EMPTY_COMMENT);
    }
    if content.starts_with(KEPT_AFTER_THE_HASH) {
        return Cow::Borrowed(trimmed);
    }
    if !content.starts_with(NO_BREAK_SPACE) {
        return Cow::Owned(format!("# {content}"));
    }
    let without_no_break_spaces = content.trim_start_matches(NO_BREAK_SPACE);
    if without_no_break_spaces.trim_start().starts_with(TYPE_PRAGMA) {
        Cow::Owned(format!("# {content}"))
    } else if without_no_break_spaces.starts_with(' ') {
        Cow::Owned(format!("# {without_no_break_spaces}"))
    } else {
        let rest = content.get(NO_BREAK_SPACE.len_utf8()..).unwrap_or_default();
        Cow::Owned(format!("# {rest}"))
    }
}

const fn expected_empty_lines(f: &Formatter<'_>) -> usize {
    if f.node_level.is_top_level() {
        TOP_LEVEL_EMPTY_LINES
    } else {
        NESTED_EMPTY_LINES
    }
}

// Después de una función o una clase, un comentario en su propia línea queda separado por las
// mismas líneas en blanco que una sentencia.
pub fn empty_lines_before_trailing_comments(f: &mut Formatter<'_>, comments: &[SourceComment]) {
    let Some(comment) = comments.iter().find(|comment| comment.is_own_line()) else {
        return;
    };
    let actual = lines_before(f.source, comment.span.start).saturating_sub(1);
    for _ in actual..expected_empty_lines(f) {
        f.line(LineMode::Empty);
    }
}

pub fn empty_lines_after_leading_comments(f: &mut Formatter<'_>, comments: &[SourceComment]) {
    let Some(comment) = comments.iter().rev().find(|comment| comment.is_own_line()) else {
        return;
    };
    let actual = lines_after(f.source, comment.span.end).saturating_sub(1);
    if actual == 0 {
        return;
    }
    for _ in actual..expected_empty_lines(f) {
        f.line(LineMode::Empty);
    }
}
