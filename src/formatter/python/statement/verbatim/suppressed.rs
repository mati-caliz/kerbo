use super::lines::{Indentation, write_verbatim_range};
use super::ranges::{Range, RangeIter};
use super::{
    SUPPRESSION, Suite, is_off, mark_handled, mark_verbatim_comments_formatted, trailing_semicolon,
    write_from_trailing,
};
use crate::formatter::document::LineMode;
use crate::formatter::python::ast::{NodeId, Span};
use crate::formatter::python::comments::SourceComment;
use crate::formatter::python::comments::format::{format_comment, leading_comments, trailing_comments};
use crate::formatter::python::comments::placement::branches::last_child_in_body;
use crate::formatter::python::context::{Formatted, Formatter, Unsupported};
use crate::formatter::python::statement::suite::fmt_suite_child;

// Cómo sigue un tramo apagado después de repasar los comentarios de una sentencia.
enum Next<'a> {
    Suppressed(&'a SourceComment),
    Finished,
}

#[derive(Clone, Copy)]
struct Position {
    index: usize,
    indentation: Indentation,
}

fn restart_after_on<'a>(
    f: &mut Formatter<'_>,
    new_off: Option<&'a SourceComment>,
) -> Option<&'a SourceComment> {
    let new_off = new_off?;
    format_comment(f, new_off);
    new_off.mark_formatted();
    mark_handled(f, new_off);
    Some(new_off)
}

fn statement_at(suite: &Suite<'_>, index: usize) -> NodeId {
    suite.statements.get(index).copied().unwrap_or_default()
}

fn write_leading_ranges<'a>(
    f: &mut Formatter<'a>,
    (suite, position): (&Suite<'_>, Position),
    (format_off, leading): (&'a SourceComment, &'a [SourceComment]),
) -> Formatted<Next<'a>> {
    let statement = statement_at(suite, position.index);
    let mut format_off = format_off;
    for range in RangeIter::in_suppression(leading, f.source) {
        match range {
            Range::Suppressed(suppressed) => suppressed.iter().for_each(SourceComment::mark_formatted),
            Range::Ends {
                suppressed,
                on,
                formatted,
                off,
            } => {
                suppressed.iter().for_each(SourceComment::mark_formatted);
                write_verbatim_range(
                    f,
                    Span::new(format_off.span.end, on.span.start),
                    position.indentation,
                );
                mark_handled(f, on);
                leading_comments(f, std::slice::from_ref(on));
                leading_comments(f, formatted);
                match restart_after_on(f, off) {
                    Some(new_off) => format_off = new_off,
                    None => return finish_leading(f, suite, position),
                }
            }
            Range::Starts { .. } | Range::Formatted => {
                return Err(Unsupported::at(f.ast.start(statement), SUPPRESSION));
            }
        }
    }
    Ok(Next::Suppressed(format_off))
}

fn finish_leading<'a>(f: &mut Formatter<'a>, suite: &Suite<'_>, position: Position) -> Formatted<Next<'a>> {
    let statement = statement_at(suite, position.index);
    if f.comments
        .trailing(statement)
        .iter()
        .any(|comment| is_off(f, comment))
    {
        write_from_trailing(f, suite, position.index)?;
    } else {
        fmt_suite_child(f, suite.statements, position.index, suite.kind)?;
    }
    Ok(Next::Finished)
}

fn write_trailing_ranges<'a>(
    f: &mut Formatter<'a>,
    (suite, position): (&Suite<'_>, Position),
    format_off: &'a SourceComment,
) -> Formatted<Next<'a>> {
    let statement = statement_at(suite, position.index);
    let mut format_off = format_off;
    for range in RangeIter::in_suppression(f.comments.trailing(statement), f.source) {
        match range {
            Range::Suppressed(suppressed) => suppressed.iter().for_each(SourceComment::mark_formatted),
            Range::Ends {
                suppressed,
                on,
                formatted,
                off,
            } => {
                suppressed.iter().for_each(SourceComment::mark_formatted);
                write_verbatim_range(
                    f,
                    Span::new(format_off.span.end, on.span.start),
                    position.indentation,
                );
                format_comment(f, on);
                f.line(LineMode::Hard);
                trailing_comments(f, formatted);
                on.mark_formatted();
                mark_handled(f, on);
                match restart_after_on(f, off) {
                    Some(new_off) => format_off = new_off,
                    None => return Ok(Next::Finished),
                }
            }
            Range::Starts { .. } | Range::Formatted => {
                return Err(Unsupported::at(f.ast.start(statement), SUPPRESSION));
            }
        }
    }
    Ok(Next::Suppressed(format_off))
}

pub fn write_suppressed<'a>(
    f: &mut Formatter<'a>,
    suite: &Suite<'_>,
    (start_index, format_off): (usize, &'a SourceComment),
    first_leading: &'a [SourceComment],
) -> Formatted<usize> {
    let first = statement_at(suite, start_index);
    let indentation = Indentation::of_line_with(f.source, f.ast.start(first));
    let mut position = Position {
        index: start_index,
        indentation,
    };
    let mut format_off = format_off;
    let mut leading = first_leading;
    loop {
        let statement = statement_at(suite, position.index);
        match write_leading_ranges(f, (suite, position), (format_off, leading))? {
            Next::Suppressed(off) => format_off = off,
            Next::Finished => return Ok(position.index),
        }
        mark_verbatim_comments_formatted(f, statement);
        match write_trailing_ranges(f, (suite, position), format_off)? {
            Next::Suppressed(off) => format_off = off,
            Next::Finished => return Ok(position.index),
        }
        let Some(next) = suite.statements.get(position.index.saturating_add(1)) else {
            let end = end_of_last_statement(f, statement);
            write_verbatim_range(f, Span::new(format_off.span.end, end), indentation);
            return Ok(position.index);
        };
        position.index = position.index.saturating_add(1);
        leading = f.comments.leading(*next);
    }
}

fn end_of_last_statement(f: &Formatter<'_>, statement: NodeId) -> usize {
    let mut current = statement;
    loop {
        if let Some(comment) = f.comments.trailing(current).last() {
            return comment.span.end;
        }
        if let Some(child) = last_child_in_body(f.ast, current) {
            current = child;
        } else {
            return trailing_semicolon(f, current)
                .map_or_else(|| f.ast.end(statement), |semicolon| semicolon.end);
        }
    }
}
