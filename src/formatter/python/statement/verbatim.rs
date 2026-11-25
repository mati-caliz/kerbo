mod lines;
mod ranges;
mod suppressed;

use super::suite::{SuiteKind, fmt_suite_child};
use crate::formatter::python::ast::{NodeId, Span};
use crate::formatter::python::comments::SourceComment;
use crate::formatter::python::comments::format::{
    empty_lines, format_comment, leading_comments, trailing_comments,
};
use crate::formatter::python::comments::suppression::{SuppressionKind, kind_of};
use crate::formatter::python::context::{Formatted, Formatter, Unsupported};
use crate::formatter::python::trivia::{TokenKind, Tokenizer, lines_before};
use lines::{Indentation, write_verbatim_range, write_verbatim_text};
use ranges::{Range, RangeIter};
use suppressed::write_suppressed;

pub(super) const SUPPRESSION: &str = "comentario que apaga el formateo";

fn kind_of_comment(f: &Formatter<'_>, comment: &SourceComment) -> Option<SuppressionKind> {
    kind_of(f.text_of(comment.span))
}

fn is_off_in(source: &str, comment: &SourceComment) -> bool {
    comment.is_own_line()
        && kind_of(
            source
                .get(comment.span.start..comment.span.end)
                .unwrap_or_default(),
        ) == Some(SuppressionKind::Off)
}

fn is_on_in(source: &str, comment: &SourceComment) -> bool {
    comment.is_own_line()
        && kind_of(
            source
                .get(comment.span.start..comment.span.end)
                .unwrap_or_default(),
        ) == Some(SuppressionKind::On)
}

pub(super) fn is_off(f: &Formatter<'_>, comment: &SourceComment) -> bool {
    is_off_in(f.source, comment)
}

fn has_skip_comment(f: &Formatter<'_>, comments: &[SourceComment]) -> bool {
    comments.iter().any(|comment| {
        !comment.is_own_line()
            && matches!(
                kind_of_comment(f, comment),
                Some(SuppressionKind::Skip | SuppressionKind::Off)
            )
    })
}

pub(super) fn mark_handled(f: &mut Formatter<'_>, comment: &SourceComment) {
    f.handled_suppressions.push(comment.span.start);
}

pub(super) fn mark_verbatim_comments_formatted(f: &Formatter<'_>, node: NodeId) {
    for comment in f.comments.dangling(node) {
        comment.mark_formatted();
    }
    let mut pending = f.ast.children(node);
    while let Some(child) = pending.pop() {
        for comment in f
            .comments
            .leading(child)
            .iter()
            .chain(f.comments.dangling(child))
            .chain(f.comments.trailing(child))
        {
            comment.mark_formatted();
        }
        pending.extend(f.ast.children(child));
    }
}

pub fn trailing_semicolon(f: &Formatter<'_>, statement: NodeId) -> Option<Span> {
    Tokenizer::starts_at(f.ast.end(statement), f.source)
        .take_while(|token| token.kind != TokenKind::Comment)
        .find(|token| !token.kind.is_trivia())
        .filter(|token| token.kind == TokenKind::Semi)
        .map(|token| token.span)
}

fn new_logical_line_between(f: &Formatter<'_>, between: Span) -> bool {
    let mut tokens = Tokenizer::new(f.source, between);
    while let Some(token) = tokens.next() {
        if token.kind == TokenKind::Continuation {
            tokens.next();
        } else if token.kind == TokenKind::Newline {
            return true;
        }
    }
    false
}

fn skip_range(f: &Formatter<'_>, statements: &[NodeId], index: usize) -> Option<Span> {
    let first = *statements.get(index)?;
    let mut last = first;
    for statement in statements.get(index.saturating_add(1)..).unwrap_or_default() {
        if new_logical_line_between(f, Span::new(f.ast.end(last), f.ast.start(*statement))) {
            break;
        }
        last = *statement;
    }
    has_skip_comment(f, f.comments.trailing(last)).then(|| {
        let end = trailing_semicolon(f, last).map_or_else(|| f.ast.end(last), |semicolon| semicolon.end);
        Span::new(f.ast.start(first), end)
    })
}

fn write_skipped(
    f: &mut Formatter<'_>,
    statements: &[NodeId],
    index: usize,
    range: Span,
) -> Formatted<usize> {
    let comments = f.comments;
    let first = statements
        .get(index)
        .copied()
        .ok_or_else(|| Unsupported::at(range.start, SUPPRESSION))?;
    mark_verbatim_comments_formatted(f, first);
    let mut last_index = index;
    while let Some(next) = statements.get(last_index.saturating_add(1)) {
        if f.ast.end(*next) > range.end {
            break;
        }
        mark_verbatim_comments_formatted(f, *next);
        last_index = last_index.saturating_add(1);
    }
    let preceding = statements.get(last_index).copied().unwrap_or(first);
    for comment in comments.trailing(preceding) {
        if has_skip_comment(f, std::slice::from_ref(comment)) {
            mark_handled(f, comment);
        }
    }
    leading_comments(f, comments.leading(first));
    write_verbatim_text(f, range);
    trailing_comments(f, comments.trailing(preceding));
    Ok(last_index)
}

fn write_trailing_off_comment(f: &mut Formatter<'_>, comment: &SourceComment) {
    let lines = lines_before(f.source, comment.span.start);
    empty_lines(f, lines);
    format_comment(f, comment);
    comment.mark_formatted();
    mark_handled(f, comment);
}

pub(super) struct Suite<'a> {
    pub statements: &'a [NodeId],
    pub kind: SuiteKind,
}

fn write_from_leading(f: &mut Formatter<'_>, suite: &Suite<'_>, index: usize) -> Formatted<usize> {
    let first = suite
        .statements
        .get(index)
        .copied()
        .ok_or_else(|| Unsupported::at(0, SUPPRESSION))?;
    let mut ranges = RangeIter::outside_suppression(f.comments.leading(first), f.source);
    let Some(Range::Starts { formatted, off }) = ranges.next() else {
        return Err(Unsupported::at(f.ast.start(first), SUPPRESSION));
    };
    leading_comments(f, formatted);
    format_comment(f, off);
    off.mark_formatted();
    mark_handled(f, off);
    write_suppressed(f, suite, (index, off), ranges.remaining())
}

pub(super) fn write_from_trailing(
    f: &mut Formatter<'_>,
    suite: &Suite<'_>,
    index: usize,
) -> Formatted<usize> {
    let comments = f.comments;
    let last_formatted = suite
        .statements
        .get(index)
        .copied()
        .ok_or_else(|| Unsupported::at(0, SUPPRESSION))?;
    let indentation = Indentation::of_line_with(f.source, f.ast.start(last_formatted));
    let trailing = comments.trailing(last_formatted);
    let mut ranges = RangeIter::outside_suppression(trailing, f.source);
    let Some(Range::Starts { off, .. }) = ranges.next() else {
        return Err(Unsupported::at(f.ast.start(last_formatted), SUPPRESSION));
    };
    let mut format_off = off;
    ranges.remaining().iter().for_each(SourceComment::mark_formatted);
    format_off.mark_formatted();
    fmt_suite_child(f, suite.statements, index, suite.kind)?;
    format_off.mark_unformatted();
    write_trailing_off_comment(f, format_off);
    for range in ranges {
        match range {
            Range::Ends {
                on,
                formatted,
                off: new_off,
                ..
            } => {
                on.mark_unformatted();
                formatted.iter().for_each(SourceComment::mark_unformatted);
                write_verbatim_range(f, Span::new(format_off.span.end, on.span.start), indentation);
                mark_handled(f, on);
                trailing_comments(f, std::slice::from_ref(on));
                trailing_comments(f, formatted);
                let Some(new_off) = new_off else {
                    return Ok(index);
                };
                new_off.mark_unformatted();
                write_trailing_off_comment(f, new_off);
                format_off = new_off;
            }
            Range::Suppressed(_) => {}
            Range::Starts { .. } | Range::Formatted => {
                return Err(Unsupported::at(f.ast.start(last_formatted), SUPPRESSION));
            }
        }
    }
    if let Some(next) = suite.statements.get(index.saturating_add(1)) {
        return write_suppressed(
            f,
            suite,
            (index.saturating_add(1), format_off),
            comments.leading(*next),
        );
    }
    if let Some(last_comment) = trailing.last() {
        write_verbatim_range(
            f,
            Span::new(format_off.span.end, last_comment.span.end),
            indentation,
        );
    }
    Ok(index)
}

// Si las sentencias desde `index` se escriben tal como están (`fmt: skip` o `fmt: off`), las
// escribe y devuelve el índice de la última.
pub fn write_if_suppressed(
    f: &mut Formatter<'_>,
    statements: &[NodeId],
    (index, kind): (usize, SuiteKind),
) -> Formatted<Option<usize>> {
    let Some(first) = statements.get(index).copied() else {
        return Ok(None);
    };
    if let Some(range) = skip_range(f, statements, index) {
        return write_skipped(f, statements, index, range).map(Some);
    }
    let suite = Suite { statements, kind };
    if f.comments.leading(first).iter().any(|comment| is_off(f, comment)) {
        return write_from_leading(f, &suite, index).map(Some);
    }
    if f.comments
        .trailing(first)
        .iter()
        .any(|comment| is_off(f, comment))
    {
        return write_from_trailing(f, &suite, index).map(Some);
    }
    Ok(None)
}
