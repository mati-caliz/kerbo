use super::Placer;
use crate::formatter::python::ast::{Expr, NodeId, Other, Span, Stmt};
use crate::formatter::python::comments::visitor::{DecoratedComment, Placement};
use crate::formatter::python::trivia::{TokenKind, Tokenizer, contains_line_break};

// `@decorador\n# comentario\ndef f():` queda suelto en la función en vez de ser de los parámetros.
fn handle_leading_function_with_decorators_comment(
    placer: &Placer<'_>,
    comment: &DecoratedComment,
) -> Placement {
    let follows_a_decorator = comment
        .preceding
        .is_some_and(|preceding| matches!(placer.ast.other(preceding), Some(Other::Decorator { .. })));
    let precedes_the_parameters = comment.following.is_some_and(|following| {
        matches!(
            placer.ast.other(following),
            Some(Other::Parameters { .. } | Other::TypeParams { .. })
        )
    });
    if comment.is_own_line() && follows_a_decorator && precedes_the_parameters {
        Placement::Dangling(comment.enclosing)
    } else {
        Placement::Default
    }
}

fn handle_leading_class_with_decorators_comment(
    placer: &Placer<'_>,
    comment: &DecoratedComment,
    (decorators, name): (&[NodeId], Span),
) -> Placement {
    let follows_the_last_decorator = decorators
        .last()
        .is_some_and(|decorator| placer.end(*decorator) < comment.span.start);
    if comment.is_own_line() && comment.span.start < name.start && follows_the_last_decorator {
        Placement::Dangling(comment.enclosing)
    } else {
        Placement::Default
    }
}

fn is_between_the_keyword_and(
    placer: &Placer<'_>,
    comment: &DecoratedComment,
    first: Option<&NodeId>,
) -> bool {
    first.is_some_and(|first| {
        placer.start(comment.enclosing) < comment.span.start && comment.span.start < placer.start(*first)
    })
}

// `from a import (  # comentario` queda suelto en el import, y el comentario antes de la coma de
// un nombre queda suelto en ese nombre para no pasar al otro lado de la coma.
fn handle_import_from_comment(
    placer: &Placer<'_>,
    comment: &DecoratedComment,
    names: &[NodeId],
) -> Placement {
    if comment.is_end_of_line() && is_between_the_keyword_and(placer, comment, names.first()) {
        return Placement::Dangling(comment.enclosing);
    }
    let precedes_a_comma = Tokenizer::starts_at(comment.span.start, placer.source)
        .skip_trivia()
        .next()
        .is_some_and(|token| token.kind == TokenKind::Comma);
    match comment.preceding {
        Some(preceding)
            if precedes_a_comma && matches!(placer.ast.other(preceding), Some(Other::Alias { .. })) =>
        {
            Placement::Dangling(preceding)
        }
        Some(_) | None => Placement::Default,
    }
}

fn handle_with_comment(placer: &Placer<'_>, comment: &DecoratedComment, items: &[NodeId]) -> Placement {
    if comment.is_end_of_line() && is_between_the_keyword_and(placer, comment, items.first()) {
        Placement::Dangling(comment.enclosing)
    } else {
        Placement::Default
    }
}

fn last_two_parts(placer: &Placer<'_>, string: NodeId) -> Option<(NodeId, NodeId)> {
    let Some(Expr::StringLiteral { parts } | Expr::BytesLiteral { parts }) = placer.ast.expr(string) else {
        return None;
    };
    match parts.as_slice() {
        [.., second_last, last] => Some((*second_last, *last)),
        [] | [_] => None,
    }
}

// En `a = (\n "b"\n "c"  # comentario\n)` el comentario es de la última parte de la cadena, no
// de la cadena entera: si no, la asignación lo llevaría al final.
fn handle_trailing_implicit_concatenated_string_comment(
    placer: &Placer<'_>,
    comment: &DecoratedComment,
) -> Placement {
    if comment.is_own_line() {
        return Placement::Default;
    }
    let Some(string) = comment.preceding else {
        return Placement::Default;
    };
    let Some((second_last, last)) = last_two_parts(placer, string) else {
        return Placement::Default;
    };
    let is_broken_and_parenthesized =
        contains_line_break(placer.source, placer.end(second_last), placer.start(last))
            && placer.parenthesized.contains(&placer.ast.span(string));
    let is_inside_the_parentheses =
        !placer.has_token((placer.end(last), comment.span.start), TokenKind::RParen);
    if is_broken_and_parenthesized && is_inside_the_parentheses {
        Placement::Trailing(last)
    } else {
        Placement::Default
    }
}

const fn assigned_value(statement: &Stmt) -> Option<NodeId> {
    match statement {
        Stmt::Assign { value, .. } | Stmt::AugAssign { value, .. } => Some(*value),
        Stmt::AnnAssign { value, .. } => *value,
        Stmt::FunctionDef { .. }
        | Stmt::ClassDef { .. }
        | Stmt::Return { .. }
        | Stmt::Delete { .. }
        | Stmt::For { .. }
        | Stmt::While { .. }
        | Stmt::If { .. }
        | Stmt::With { .. }
        | Stmt::Raise { .. }
        | Stmt::Try { .. }
        | Stmt::Assert { .. }
        | Stmt::Import { .. }
        | Stmt::ImportFrom { .. }
        | Stmt::Global { .. }
        | Stmt::Nonlocal { .. }
        | Stmt::Expression { .. }
        | Stmt::TypeAlias { .. }
        | Stmt::Pass
        | Stmt::Break
        | Stmt::Continue => None,
    }
}

pub fn handle(placer: &Placer<'_>, comment: &DecoratedComment) -> Placement {
    match placer.ast.stmt(comment.enclosing) {
        Some(Stmt::FunctionDef { .. }) => handle_leading_function_with_decorators_comment(placer, comment),
        Some(Stmt::ClassDef { decorators, name, .. }) => {
            handle_leading_class_with_decorators_comment(placer, comment, (decorators, *name))
        }
        Some(Stmt::ImportFrom { names, .. }) => handle_import_from_comment(placer, comment, names),
        Some(Stmt::With { items, .. }) => handle_with_comment(placer, comment, items),
        Some(Stmt::Return { .. }) => handle_trailing_implicit_concatenated_string_comment(placer, comment),
        Some(assignment)
            if comment.preceding.is_some() && assigned_value(assignment) == comment.preceding =>
        {
            handle_trailing_implicit_concatenated_string_comment(placer, comment)
        }
        Some(_) | None => Placement::Default,
    }
}
