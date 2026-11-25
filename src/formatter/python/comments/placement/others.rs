use super::bodies::is_separated_by_an_empty_line;
use super::{Placer, handle_bracketed_end_of_line_comment};
use crate::formatter::python::ast::{NodeId, Other, SequenceKind, Span, Stmt};
use crate::formatter::python::comments::visitor::{DecoratedComment, Placement};
use crate::formatter::python::statement::parameters::separators::{
    assign_argument_separator_comment_placement, find_parameter_separators,
};
use crate::formatter::python::trivia::{TokenKind, first_non_trivia_token};

const ALIAS_NAME_COMMENT: &str = "comentario entre el nombre importado y su alias";
const OPENING_PARENTHESIS: char = '(';

// Los comentarios del `/` y del `*` sueltos quedan en los parámetros, que no tienen otro nodo
// donde colgarlos.
fn handle_parameters_separator_comment(placer: &Placer<'_>, comment: &DecoratedComment) -> Placement {
    let (slash, star) = find_parameter_separators(placer.ast, placer.source, comment.enclosing);
    let location = assign_argument_separator_comment_placement(
        slash.as_ref(),
        star.as_ref(),
        (comment.span, comment.line_position),
    );
    if location.is_some() {
        Placement::Dangling(comment.enclosing)
    } else {
        Placement::Default
    }
}

fn handle_parameters_comment(placer: &Placer<'_>, comment: &DecoratedComment) -> Placement {
    handle_parameters_separator_comment(placer, comment).or_else(|| {
        let is_parenthesized = placer
            .source
            .get(placer.start(comment.enclosing)..)
            .is_some_and(|text| text.starts_with(OPENING_PARENTHESIS));
        if is_parenthesized {
            handle_bracketed_end_of_line_comment(placer, comment)
        } else {
            Placement::Default
        }
    })
}

// El nombre de un parámetro no es un nodo: lo que está antes del nombre o de los dos puntos de
// la anotación pasa a ser del parámetro.
fn handle_parameter_comment(
    placer: &Placer<'_>,
    comment: &DecoratedComment,
    (name, annotation): (Span, Option<NodeId>),
) -> Placement {
    if annotation.is_some() {
        let is_before_the_colon = first_non_trivia_token(placer.source, name.end)
            .is_some_and(|colon| comment.span.start < colon.span.start);
        return if is_before_the_colon {
            Placement::Leading(comment.enclosing)
        } else {
            Placement::Default
        };
    }
    if comment.span.start >= name.start {
        return Placement::Default;
    }
    let parameters_starting_here = comment.enclosing_parent.filter(|parent| {
        matches!(placer.ast.other(*parent), Some(Other::Parameters { .. }))
            && placer.start(*parent) == placer.start(comment.enclosing)
    });
    Placement::Leading(parameters_starting_here.unwrap_or(comment.enclosing))
}

// Entre el nombre de un argumento y su valor, el comentario es del argumento entero, salvo que
// esté adentro de los paréntesis del valor.
fn handle_keyword_comment(placer: &Placer<'_>, comment: &DecoratedComment, arg: Option<Span>) -> Placement {
    let start = arg.map_or_else(|| placer.start(comment.enclosing), |arg| arg.end);
    if placer.has_token((start, comment.span.start), TokenKind::LParen) {
        Placement::Default
    } else {
        Placement::Leading(comment.enclosing)
    }
}

fn is_around_an_if(
    placer: &Placer<'_>,
    comment: &DecoratedComment,
    (last_end, condition): (usize, NodeId),
) -> bool {
    let condition_start = placer.start(condition);
    let Some(if_token) = placer.token_after_closing_parentheses(last_end, condition_start) else {
        return false;
    };
    let comment_start = comment.span.start;
    if comment.is_own_line() {
        last_end < comment_start && comment_start < if_token.span.start
    } else {
        if_token.span.start < comment_start && comment_start < condition_start
    }
}

// Los comentarios pegados al `for`, al `in` y a los `if` de una comprensión quedan sueltos en
// ella: esas palabras no son nodos.
fn handle_comprehension_comment(
    placer: &Placer<'_>,
    comment: &DecoratedComment,
    (target, iter, ifs): (NodeId, NodeId, &[NodeId]),
) -> Placement {
    let dangling_if = |is_dangling: bool| {
        if is_dangling {
            Placement::Dangling(comment.enclosing)
        } else {
            Placement::Default
        }
    };
    if comment.span.end < placer.start(target) {
        return dangling_if(comment.is_end_of_line());
    }
    let Some(in_token) = placer.token_after_closing_parentheses(placer.end(target), placer.start(iter))
    else {
        return Placement::Default;
    };
    if comment.span.start < in_token.span.start {
        return dangling_if(comment.is_own_line());
    }
    if comment.span.start < placer.start(iter) {
        return dangling_if(comment.is_end_of_line());
    }
    let mut last_end = placer.end(iter);
    for condition in ifs {
        if is_around_an_if(placer, comment, (last_end, *condition)) {
            return Placement::Dangling(comment.enclosing);
        }
        last_end = placer.end(*condition);
    }
    Placement::Default
}

// `with a  # del contexto\n as  # suelto\n b`.
fn handle_with_item_comment(placer: &Placer<'_>, comment: &DecoratedComment) -> Placement {
    let (Some(context_expr), Some(optional_vars)) = (comment.preceding, comment.following) else {
        return Placement::Default;
    };
    let as_token =
        placer.token_after_closing_parentheses(placer.end(context_expr), placer.start(optional_vars));
    if as_token.is_some_and(|as_token| comment.span.end < as_token.span.start) {
        Placement::Trailing(context_expr)
    } else if comment.is_end_of_line() {
        Placement::Dangling(comment.enclosing)
    } else {
        Placement::Leading(optional_vars)
    }
}

// Lo que queda después de la última sentencia del archivo es de esa sentencia.
fn handle_trailing_module_comment(comment: &DecoratedComment, body: &[NodeId]) -> Placement {
    if comment.preceding.is_some() || comment.following.is_some() {
        return Placement::Default;
    }
    body.last().map_or(Placement::Leading(comment.enclosing), |last| {
        Placement::Trailing(*last)
    })
}

// Antes de una función o una clase del módulo van dos líneas en blanco: el comentario que no
// está pegado a la definición es de la sentencia anterior, para no quedar del otro lado.
fn handle_module_level_own_line_comment_before_class_or_function_comment(
    placer: &Placer<'_>,
    comment: &DecoratedComment,
) -> Placement {
    let (Some(preceding), Some(following)) = (comment.preceding, comment.following) else {
        return Placement::Default;
    };
    let is_definition = matches!(
        placer.ast.stmt(following),
        Some(Stmt::FunctionDef { .. } | Stmt::ClassDef { .. })
    );
    if comment.is_end_of_line() || !is_definition {
        return Placement::Default;
    }
    if is_separated_by_an_empty_line(placer, comment, following) {
        Placement::Trailing(preceding)
    } else {
        Placement::Leading(following)
    }
}

// ruff cuelga estos comentarios de los nombres de un alias, que acá no son nodos: el archivo
// queda como está.
fn handle_alias_comment(placer: &Placer<'_>, name: Span, asname: Option<Span>) -> Placement {
    let follows_an_as =
        first_non_trivia_token(placer.source, name.end).is_some_and(|token| token.kind == TokenKind::As);
    if asname.is_some() && follows_an_as {
        Placement::Unsupported(ALIAS_NAME_COMMENT)
    } else {
        Placement::Default
    }
}

pub fn handle(placer: &Placer<'_>, comment: &DecoratedComment) -> Placement {
    match placer.ast.other(comment.enclosing) {
        Some(Other::Parameters { .. }) => handle_parameters_comment(placer, comment),
        Some(Other::Parameter { name, annotation }) => {
            handle_parameter_comment(placer, comment, (*name, *annotation))
        }
        Some(
            Other::Arguments { .. }
            | Other::TypeParams { .. }
            | Other::PatternArguments { .. }
            | Other::PatternSequence {
                kind: SequenceKind::List | SequenceKind::Tuple,
                ..
            },
        ) => handle_bracketed_end_of_line_comment(placer, comment),
        Some(Other::Comprehension {
            target, iter, ifs, ..
        }) => handle_comprehension_comment(placer, comment, (*target, *iter, ifs)),
        Some(Other::Keyword { arg, .. }) => handle_keyword_comment(placer, comment, *arg),
        Some(Other::Module { body }) => handle_trailing_module_comment(comment, body).or_else(|| {
            handle_module_level_own_line_comment_before_class_or_function_comment(placer, comment)
        }),
        Some(Other::WithItem { .. }) => handle_with_item_comment(placer, comment),
        Some(Other::Alias { name, asname }) => handle_alias_comment(placer, *name, *asname),
        Some(_) | None => Placement::Default,
    }
}
