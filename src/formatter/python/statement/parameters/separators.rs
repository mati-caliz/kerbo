use crate::formatter::python::ast::{Ast, NodeId, Other, Span};
use crate::formatter::python::comments::LinePosition;
use crate::formatter::python::trivia::{Token, TokenKind, Tokenizer};

// Un `/` o un `*` suelto entre los parámetros: dónde termina lo de antes y dónde empieza lo de
// después, para saber qué comentarios le pertenecen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParameterSeparator {
    pub preceding_end: usize,
    pub separator: Span,
    pub following_start: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeparatorCommentLocation {
    SlashLeading,
    SlashTrailing,
    StarLeading,
    StarTrailing,
}

impl SeparatorCommentLocation {
    pub const fn is_on_the_slash(self) -> bool {
        matches!(self, Self::SlashLeading | Self::SlashTrailing)
    }
}

fn tokens(source: &str, start: usize, end: usize) -> impl Iterator<Item = Token> + '_ {
    Tokenizer::new(source, Span::new(start, end)).skip_trivia()
}

fn token_after_the_comma(source: &str, preceding_end: usize, end: usize) -> Option<Token> {
    tokens(source, preceding_end, end).nth(1)
}

fn star_without_preceding(source: &str, span: Span) -> Option<Token> {
    let mut tokens = tokens(source, span.start, span.end);
    let first = tokens.next()?;
    if first.kind == TokenKind::LParen {
        tokens.next()
    } else {
        Some(first)
    }
}

// `find_parameter_separators` de ruff: el `/` y el `*` de `f(a, /, b, *, c)`. Si hay `*args`, los
// comentarios van con ese parámetro y no hace falta el `*`.
pub fn find_parameter_separators(
    ast: &Ast,
    source: &str,
    parameters: NodeId,
) -> (Option<ParameterSeparator>, Option<ParameterSeparator>) {
    let Some(Other::Parameters {
        posonlyargs,
        args,
        vararg,
        kwonlyargs,
        kwarg,
    }) = ast.other(parameters)
    else {
        return (None, None);
    };
    let span = ast.span(parameters);
    let slash = posonlyargs.last().and_then(|last| {
        let preceding_end = ast.end(*last);
        token_after_the_comma(source, preceding_end, span.end).map(|slash| (preceding_end, slash.span))
    });
    let star = match (vararg, kwonlyargs.first()) {
        (None, Some(first_keyword_only)) => {
            let after_parameters = args
                .last()
                .map(|last| ast.end(*last))
                .or_else(|| slash.map(|(_, slash)| slash.end));
            let found = after_parameters.map_or_else(
                || star_without_preceding(source, span).map(|star| (span.start, star)),
                |preceding_end| {
                    token_after_the_comma(source, preceding_end, span.end).map(|star| (preceding_end, star))
                },
            );
            found.map(|(preceding_end, star)| ParameterSeparator {
                preceding_end,
                separator: star.span,
                following_start: ast.start(*first_keyword_only),
            })
        }
        (Some(_), Some(_) | None) | (None, None) => None,
    };
    let slash_following_start = args
        .first()
        .or(vararg.as_ref())
        .map(|following| ast.start(*following))
        .or_else(|| star.map(|star| star.separator.start))
        .or_else(|| kwarg.map(|kwarg| ast.start(kwarg)))
        .unwrap_or(span.end);
    let slash = slash.map(|(preceding_end, separator)| ParameterSeparator {
        preceding_end,
        separator,
        following_start: slash_following_start,
    });
    (slash, star)
}

fn position_around(
    separator: &ParameterSeparator,
    (comment, position): (Span, LinePosition),
) -> Option<LinePosition> {
    let is_before = comment.start > separator.preceding_end && comment.start < separator.separator.start;
    let is_after = comment.start > separator.separator.end && comment.start < separator.following_start;
    if is_before && position == LinePosition::OwnLine {
        Some(LinePosition::OwnLine)
    } else if is_after && position == LinePosition::EndOfLine {
        Some(LinePosition::EndOfLine)
    } else {
        None
    }
}

// Un comentario en su propia línea antes del separador es suyo, y también el que queda al final
// de la línea del separador.
pub fn assign_argument_separator_comment_placement(
    slash: Option<&ParameterSeparator>,
    star: Option<&ParameterSeparator>,
    comment: (Span, LinePosition),
) -> Option<SeparatorCommentLocation> {
    let on_the_slash =
        slash
            .and_then(|slash| position_around(slash, comment))
            .map(|position| match position {
                LinePosition::OwnLine => SeparatorCommentLocation::SlashLeading,
                LinePosition::EndOfLine => SeparatorCommentLocation::SlashTrailing,
            });
    on_the_slash.or_else(|| {
        star.and_then(|star| position_around(star, comment))
            .map(|position| match position {
                LinePosition::OwnLine => SeparatorCommentLocation::StarLeading,
                LinePosition::EndOfLine => SeparatorCommentLocation::StarTrailing,
            })
    })
}
