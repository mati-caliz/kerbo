use std::cmp::Ordering;

use super::Placer;
use super::branches::{
    innermost_last_child, is_alternative_branch_with_node, is_first_statement_in_alternate_body,
    is_first_statement_in_body, last_child_in_body,
};
use crate::formatter::python::ast::{NodeId, Span};
use crate::formatter::python::comments::visitor::{DecoratedComment, Placement};
use crate::formatter::python::trivia::{TokenKind, Tokenizer, full_line_end, indentation_at_offset};

const MORE_INDENTED_THAN_ANY_COMMENT: usize = 1;

// `for x in y:  # comentario` queda suelto en el `for`; después de un cuerpo, el comentario al
// final de la línea es de la última sentencia de ese cuerpo.
pub fn handle_end_of_line_comment_around_body(placer: &Placer<'_>, comment: &DecoratedComment) -> Placement {
    if comment.is_own_line() {
        return Placement::Default;
    }
    if let Some(following) = comment.following
        && is_first_statement_in_body(placer.ast, following, comment.enclosing)
        && placer
            .tokens(comment.span.end, placer.start(following))
            .next()
            .is_none()
    {
        return Placement::Dangling(comment.enclosing);
    }
    comment
        .preceding
        .and_then(|preceding| last_child_in_body(placer.ast, preceding))
        .map_or(Placement::Default, |last_child| {
            Placement::Trailing(innermost_last_child(placer.ast, last_child))
        })
}

// `max_empty_lines` de ruff: las líneas en blanco seguidas que hay en el texto, sin contar las
// que quedan después del primer token que no es un comentario.
fn max_empty_lines(contents: &str) -> usize {
    let mut newlines = 0_usize;
    let mut max_newlines = 0_usize;
    for token in Tokenizer::new(contents, Span::new(0, contents.len())) {
        if token.kind == TokenKind::Newline {
            newlines = newlines.saturating_add(1);
        } else if token.kind == TokenKind::Comment {
            max_newlines = max_newlines.max(newlines);
            newlines = 0;
        } else if token.kind != TokenKind::Whitespace {
            break;
        }
    }
    max_newlines.max(newlines).saturating_sub(1)
}

pub fn is_separated_by_an_empty_line(
    placer: &Placer<'_>,
    comment: &DecoratedComment,
    following: NodeId,
) -> bool {
    let between = placer
        .source
        .get(comment.span.end..placer.start(following))
        .unwrap_or_default();
    max_empty_lines(between) != 0
}

// Entre dos sentencias, el comentario pegado a la que sigue es suyo; si los separa una línea en
// blanco, es de la anterior.
pub fn handle_own_line_comment_between_statements(
    placer: &Placer<'_>,
    comment: &DecoratedComment,
) -> Placement {
    let (Some(preceding), Some(following)) = (comment.preceding, comment.following) else {
        return Placement::Default;
    };
    let both_are_statements =
        placer.ast.data(preceding).is_statement() && placer.ast.data(following).is_statement();
    if !both_are_statements || comment.is_end_of_line() {
        return Placement::Default;
    }
    if is_separated_by_an_empty_line(placer, comment, following) {
        Placement::Trailing(preceding)
    } else {
        Placement::Leading(following)
    }
}

// La sangría del comentario menos sangrado desde el renglón siguiente a `preceding`.
fn comment_indentation_after(placer: &Placer<'_>, preceding: NodeId, comment: Span) -> usize {
    let start = full_line_end(placer.source, placer.end(preceding));
    Tokenizer::new(placer.source, Span::new(start, comment.end))
        .filter(|token| token.kind == TokenKind::Comment)
        .filter_map(|token| indentation_at_offset(placer.source, token.span.start).map(str::len))
        .min()
        .unwrap_or(0)
}

fn indentation_of(placer: &Placer<'_>, node: NodeId) -> Option<usize> {
    indentation_at_offset(placer.source, placer.start(node)).map(str::len)
}

// Entre dos ramas (`else`, `except`...), la sangría decide: como la sentencia anterior, es de
// ella; menos, es de la rama que sigue.
fn handle_own_line_comment_between_branches(
    placer: &Placer<'_>,
    comment: &DecoratedComment,
    preceding: NodeId,
) -> Placement {
    let Some(following) = comment.following else {
        return Placement::Default;
    };
    if !is_first_statement_in_alternate_body(placer.ast, following, comment.enclosing) {
        return Placement::Default;
    }
    let comment_indentation = comment_indentation_after(placer, preceding, comment.span);
    let preceding_indentation = indentation_of(placer, preceding)
        .unwrap_or_else(|| comment_indentation.saturating_add(MORE_INDENTED_THAN_ANY_COMMENT));
    match comment_indentation.cmp(&preceding_indentation) {
        Ordering::Greater => Placement::Default,
        Ordering::Equal if is_alternative_branch_with_node(placer.ast, preceding) => {
            Placement::Dangling(comment.enclosing)
        }
        Ordering::Equal => Placement::Trailing(preceding),
        Ordering::Less if is_alternative_branch_with_node(placer.ast, following) => {
            Placement::Leading(following)
        }
        Ordering::Less => Placement::Dangling(comment.enclosing),
    }
}

// Baja por la última sentencia de cada cuerpo hasta la que tiene la sangría del comentario.
fn deepest_statement_with_indentation(
    placer: &Placer<'_>,
    comment_indentation: usize,
    last_child: NodeId,
) -> Placement {
    let mut parent = None;
    let mut last_child_in_parent = last_child;
    loop {
        let child_indentation = indentation_of(placer, last_child_in_parent).unwrap_or(0);
        match comment_indentation.cmp(&child_indentation) {
            Ordering::Less => return parent.map_or(Placement::Default, Placement::Trailing),
            Ordering::Equal => return Placement::Trailing(last_child_in_parent),
            Ordering::Greater => {
                let Some(nested_child) = last_child_in_body(placer.ast, last_child_in_parent) else {
                    return Placement::Trailing(last_child_in_parent);
                };
                parent = Some(last_child_in_parent);
                last_child_in_parent = nested_child;
            }
        }
    }
}

fn handle_own_line_comment_after_branch(
    placer: &Placer<'_>,
    comment: &DecoratedComment,
    preceding: NodeId,
) -> Placement {
    let follows_an_alternate_body = comment.following.is_some_and(|following| {
        is_first_statement_in_alternate_body(placer.ast, following, comment.enclosing)
    });
    let last_child = match last_child_in_body(placer.ast, preceding) {
        Some(last_child) => last_child,
        None if follows_an_alternate_body => preceding,
        None => return Placement::Default,
    };
    let comment_indentation = comment_indentation_after(placer, preceding, comment.span);
    let preceding_indentation = indentation_of(placer, preceding).unwrap_or(0);
    if comment_indentation == preceding_indentation {
        return Placement::Default;
    }
    deepest_statement_with_indentation(placer, comment_indentation, last_child)
}

// Un comentario en su propia línea al final de un cuerpo, entre dos cuerpos o después del último.
pub fn handle_own_line_comment_around_body(placer: &Placer<'_>, comment: &DecoratedComment) -> Placement {
    if comment.is_end_of_line() {
        return Placement::Default;
    }
    let Some(preceding) = comment.preceding else {
        return Placement::Default;
    };
    let has_a_token_before = placer
        .tokens(placer.end(preceding), comment.span.start)
        .next()
        .is_some();
    if has_a_token_before {
        return Placement::Default;
    }
    handle_own_line_comment_between_branches(placer, comment, preceding)
        .or_else(|| handle_own_line_comment_after_branch(placer, comment, preceding))
        .or_else(|| handle_own_line_comment_between_statements(placer, comment))
}
