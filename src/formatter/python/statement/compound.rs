use super::clause::{ClauseComments, clause};
use super::suite::SuiteKind;
use crate::formatter::python::ast::{Expr, NodeId, Other, Stmt};
use crate::formatter::python::comments::SourceComment;
use crate::formatter::python::comments::format::{dangling_comments, leading_alternate_branch_comments};
use crate::formatter::python::context::{Formatted, Formatter};
use crate::formatter::python::expression::{
    TupleParentheses, fmt_tuple, format_node_with, maybe_parenthesize_expression,
};
use crate::formatter::python::parentheses::Parenthesize;

const ELSE: &str = "else";
const COMPOUND: &str = "sentencia compuesta";

fn fmt_condition(f: &mut Formatter<'_>, keyword: &'static str, test: NodeId, parent: NodeId) -> Formatted {
    f.token(keyword);
    f.space();
    maybe_parenthesize_expression(f, test, parent, Parenthesize::IfBreaks)
}

fn fmt_elif_else_clause(f: &mut Formatter<'_>, node: NodeId, last_node: Option<NodeId>) -> Formatted {
    let Some(Other::ElifElseClause { test, body }) = f.ast.other(node) else {
        return Err(f.unsupported(node, COMPOUND));
    };
    let comments = f.comments;
    let clause_comments =
        ClauseComments::with_leading(comments.leading(node), last_node, comments.dangling(node));
    clause(f, clause_comments, (body, SuiteKind::Other), |f| {
        if let Some(test) = test {
            return fmt_condition(f, "elif", *test, node);
        }
        f.token(ELSE);
        Ok(())
    })
}

pub fn fmt_if(
    f: &mut Formatter<'_>,
    statement: NodeId,
    test: NodeId,
    (body, clauses): (&[NodeId], &[NodeId]),
) -> Formatted {
    let trailing_colon_comment = f.comments.dangling(statement);
    clause(
        f,
        ClauseComments::trailing(trailing_colon_comment),
        (body, SuiteKind::Other),
        |f| fmt_condition(f, "if", test, statement),
    )?;
    let mut last_node = body.last().copied();
    for node in clauses {
        fmt_elif_else_clause(f, *node, last_node)?;
        if let Some(Other::ElifElseClause {
            body: clause_body, ..
        }) = f.ast.other(*node)
        {
            last_node = clause_body.last().copied();
        }
    }
    Ok(())
}

// Los comentarios sueltos de un `while` o un `for`: los de después de los dos puntos y los que
// rodean al `else`.
fn split_loop_comments<'comments>(
    f: &Formatter<'comments>,
    statement: NodeId,
    body_start: usize,
) -> (&'comments [SourceComment], &'comments [SourceComment]) {
    let dangling = f.comments.dangling(statement);
    dangling.split_at(dangling.partition_point(|comment| comment.span.end < body_start))
}

fn fmt_loop_else(
    f: &mut Formatter<'_>,
    (body, orelse): (&[NodeId], &[NodeId]),
    or_else_comments: &[SourceComment],
) -> Formatted {
    if orelse.is_empty() {
        return Ok(());
    }
    let (leading, trailing) =
        or_else_comments.split_at(or_else_comments.partition_point(SourceComment::is_own_line));
    let clause_comments = ClauseComments::with_leading(leading, body.last().copied(), trailing);
    clause(f, clause_comments, (orelse, SuiteKind::Other), |f| {
        f.token(ELSE);
        Ok(())
    })
}

pub fn fmt_while(
    f: &mut Formatter<'_>,
    statement: NodeId,
    test: NodeId,
    (body, orelse): (&[NodeId], &[NodeId]),
) -> Formatted {
    let body_start = body
        .first()
        .map_or_else(|| f.ast.end(test), |first| f.ast.start(*first));
    let (trailing_condition_comments, or_else_comments) = split_loop_comments(f, statement, body_start);
    clause(
        f,
        ClauseComments::trailing(trailing_condition_comments),
        (body, SuiteKind::Other),
        |f| fmt_condition(f, "while", test, statement),
    )?;
    fmt_loop_else(f, (body, orelse), or_else_comments)
}

fn fmt_for_target(f: &mut Formatter<'_>, target: NodeId) -> Formatted {
    if matches!(f.ast.expr(target), Some(Expr::Tuple { .. })) {
        return format_node_with(f, target, |f| {
            fmt_tuple(f, target, TupleParentheses::NeverPreserve)
        });
    }
    maybe_parenthesize_expression(f, target, target, Parenthesize::IfBreaks)
}

pub fn fmt_for(f: &mut Formatter<'_>, statement: NodeId) -> Formatted {
    let Some(Stmt::For {
        is_async,
        target,
        iter,
        body,
        orelse,
    }) = f.ast.stmt(statement)
    else {
        return Err(f.unsupported(statement, COMPOUND));
    };
    let body_start = body
        .first()
        .map_or_else(|| f.ast.end(*iter), |first| f.ast.start(*first));
    let (trailing_condition_comments, or_else_comments) = split_loop_comments(f, statement, body_start);
    clause(
        f,
        ClauseComments::trailing(trailing_condition_comments),
        (body, SuiteKind::Other),
        |f| {
            if *is_async {
                f.token("async");
                f.space();
            }
            f.token("for");
            f.space();
            fmt_for_target(f, *target)?;
            f.space();
            f.token("in");
            f.space();
            maybe_parenthesize_expression(f, *iter, statement, Parenthesize::IfBreaks)
        },
    )?;
    fmt_loop_else(f, (body, orelse), or_else_comments)
}

fn fmt_except_handler(f: &mut Formatter<'_>, handler: NodeId, is_star: bool) -> Formatted {
    let Some(Other::ExceptHandler { type_, name, body }) = f.ast.other(handler) else {
        return Err(f.unsupported(handler, COMPOUND));
    };
    let dangling = f.comments.dangling(handler);
    format_node_with(f, handler, |f| {
        clause(
            f,
            ClauseComments::trailing(dangling),
            (body, SuiteKind::Other),
            |f| {
                f.token("except");
                if is_star {
                    f.token("*");
                }
                let Some(type_) = type_ else {
                    return Ok(());
                };
                f.space();
                maybe_parenthesize_expression(f, *type_, handler, Parenthesize::IfBreaks)?;
                if let Some(name) = name {
                    f.space();
                    f.token("as");
                    f.space();
                    f.source_text(*name);
                }
                Ok(())
            },
        )
    })
}

// `try`, `else` y `finally`: de los comentarios sueltos de la sentencia, a cada bloque le tocan
// los que quedan antes del final de su última sentencia.
fn fmt_case<'comments>(
    f: &mut Formatter<'_>,
    (keyword, body): (&'static str, &[NodeId]),
    previous_node: Option<NodeId>,
    dangling: &'comments [SourceComment],
) -> Formatted<(Option<NodeId>, &'comments [SourceComment])> {
    let Some(last) = body.last().copied() else {
        return Ok((previous_node, dangling));
    };
    let last_end = f.ast.end(last);
    let (case_comments, rest) =
        dangling.split_at(dangling.partition_point(|comment| comment.span.end <= last_end));
    let (leading, trailing) =
        case_comments.split_at(case_comments.partition_point(SourceComment::is_own_line));
    let clause_comments = ClauseComments::with_leading(leading, previous_node, trailing);
    clause(f, clause_comments, (body, SuiteKind::Other), |f| {
        f.token(keyword);
        Ok(())
    })?;
    Ok((Some(last), rest))
}

pub fn fmt_try(f: &mut Formatter<'_>, statement: NodeId) -> Formatted {
    let Some(Stmt::Try {
        body,
        handlers,
        orelse,
        finalbody,
        is_star,
    }) = f.ast.stmt(statement)
    else {
        return Err(f.unsupported(statement, COMPOUND));
    };
    let comments = f.comments;
    let dangling = comments.dangling(statement);
    let (mut previous_node, dangling) = fmt_case(f, ("try", body), None, dangling)?;
    for handler in handlers {
        leading_alternate_branch_comments(f, comments.leading(*handler), previous_node);
        fmt_except_handler(f, *handler, *is_star)?;
        if let Some(Other::ExceptHandler {
            body: handler_body, ..
        }) = f.ast.other(*handler)
        {
            previous_node = handler_body.last().copied();
        }
    }
    let (previous_node, dangling) = fmt_case(f, (ELSE, orelse), previous_node, dangling)?;
    let (_, dangling) = fmt_case(f, ("finally", finalbody), previous_node, dangling)?;
    dangling_comments(f, dangling);
    Ok(())
}
