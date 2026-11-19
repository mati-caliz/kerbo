use super::clause::{ClauseComments, clause};
use super::parameters::{ParametersParentheses, fmt_parameters, parameters_are_empty};
use super::suite::SuiteKind;
use crate::formatter::document::LineMode;
use crate::formatter::python::ast::{Expr, NodeId, Other, Span, Stmt};
use crate::formatter::python::builders::group;
use crate::formatter::python::comments::SourceComment;
use crate::formatter::python::comments::format::{
    empty_lines_after_leading_comments, empty_lines_before_trailing_comments, leading_comments,
    trailing_comments,
};
use crate::formatter::python::context::{Formatted, Formatter};
use crate::formatter::python::expression::own_parentheses::arguments_are_empty;
use crate::formatter::python::expression::{
    fmt_arguments, format_expr, format_node_with, maybe_parenthesize_expression,
};
use crate::formatter::python::parentheses::{Parentheses, Parenthesize};
use crate::formatter::python::trivia::lines_after_ignoring_end_of_line_trivia;

const DEFINITION: &str = "definición";

fn fmt_decorator(f: &mut Formatter<'_>, decorator: NodeId) -> Formatted {
    let Some(Other::Decorator { expression }) = f.ast.other(decorator) else {
        return Err(f.unsupported(decorator, DEFINITION));
    };
    format_node_with(f, decorator, |f| {
        f.token("@");
        maybe_parenthesize_expression(f, *expression, decorator, Parenthesize::Optional)
    })
}

// Los decoradores, uno por línea, y después los comentarios que quedaron entre el último y el
// `def` o `class`.
fn fmt_decorators(
    f: &mut Formatter<'_>,
    decorators: &[NodeId],
    leading_definition_comments: &[SourceComment],
) -> Formatted {
    let Some(last_decorator) = decorators.last() else {
        return Ok(());
    };
    for (index, decorator) in decorators.iter().enumerate() {
        if index > 0 {
            f.line(LineMode::Hard);
        }
        fmt_decorator(f, *decorator)?;
    }
    if leading_definition_comments.is_empty() {
        f.line(LineMode::Hard);
        return Ok(());
    }
    if lines_after_ignoring_end_of_line_trivia(f.source, f.ast.end(*last_decorator)) <= 1 {
        f.line(LineMode::Hard);
    } else {
        f.line(LineMode::Empty);
    }
    leading_comments(f, leading_definition_comments);
    Ok(())
}

fn split_definition_comments<'comments>(
    f: &Formatter<'comments>,
    statement: NodeId,
) -> (&'comments [SourceComment], &'comments [SourceComment]) {
    let dangling = f.comments.dangling(statement);
    dangling.split_at(dangling.partition_point(SourceComment::is_own_line))
}

fn fmt_return_annotation(
    f: &mut Formatter<'_>,
    statement: NodeId,
    parameters: NodeId,
    returns: NodeId,
) -> Formatted {
    f.space();
    f.token("->");
    f.space();
    let comments = f.comments;
    if matches!(f.ast.expr(returns), Some(Expr::Tuple { .. })) {
        let parentheses = if comments.has_leading(returns) {
            Parentheses::Always
        } else {
            Parentheses::Never
        };
        return format_expr(f, returns, parentheses);
    }
    if comments.has_trailing(returns) {
        return format_expr(f, returns, Parentheses::Always);
    }
    let parenthesize = if parameters_are_empty(f, parameters) && !comments.has(parameters) {
        Parenthesize::IfBreaksParenthesized
    } else {
        Parenthesize::IfBreaks
    };
    maybe_parenthesize_expression(f, returns, statement, parenthesize)
}

pub fn fmt_function(f: &mut Formatter<'_>, statement: NodeId) -> Formatted {
    let Some(Stmt::FunctionDef {
        is_async,
        decorators,
        name,
        type_params,
        parameters,
        returns,
        body,
    }) = f.ast.stmt(statement)
    else {
        return Err(f.unsupported(statement, DEFINITION));
    };
    if let Some(type_params) = type_params {
        return Err(f.unsupported(*type_params, "parámetros de tipo"));
    }
    let comments = f.comments;
    let (leading_definition_comments, trailing_definition_comments) = split_definition_comments(f, statement);
    empty_lines_after_leading_comments(f, comments.leading(statement));
    fmt_decorators(f, decorators, leading_definition_comments)?;
    clause(
        f,
        ClauseComments::trailing(trailing_definition_comments),
        (body, SuiteKind::Function),
        |f| {
            if *is_async {
                f.token("async");
                f.space();
            }
            f.token("def");
            f.space();
            f.source_text(*name);
            group(f, |f| {
                format_node_with(f, *parameters, |f| {
                    fmt_parameters(f, *parameters, ParametersParentheses::Preserve)
                })?;
                returns.as_ref().map_or(Ok(()), |returns| {
                    fmt_return_annotation(f, statement, *parameters, *returns)
                })
            })
        },
    )?;
    empty_lines_before_trailing_comments(f, comments.trailing(statement));
    Ok(())
}

fn fmt_class_header(f: &mut Formatter<'_>, name: Span, arguments: Option<NodeId>) -> Formatted {
    f.token("class");
    f.space();
    f.source_text(name);
    let Some(arguments) = arguments else {
        return Ok(());
    };
    let dangling = f.comments.dangling(arguments);
    let only_end_of_line_comments = dangling.iter().all(|comment| !comment.is_own_line());
    if arguments_are_empty(f, arguments) && only_end_of_line_comments {
        trailing_comments(f, dangling);
        return Ok(());
    }
    format_node_with(f, arguments, |f| fmt_arguments(f, arguments))
}

pub fn fmt_class(f: &mut Formatter<'_>, statement: NodeId) -> Formatted {
    let Some(Stmt::ClassDef {
        decorators,
        name,
        type_params,
        arguments,
        body,
    }) = f.ast.stmt(statement)
    else {
        return Err(f.unsupported(statement, DEFINITION));
    };
    if let Some(type_params) = type_params {
        return Err(f.unsupported(*type_params, "parámetros de tipo"));
    }
    let comments = f.comments;
    let (leading_definition_comments, trailing_definition_comments) = split_definition_comments(f, statement);
    empty_lines_after_leading_comments(f, comments.leading(statement));
    fmt_decorators(f, decorators, leading_definition_comments)?;
    clause(
        f,
        ClauseComments::trailing(trailing_definition_comments),
        (body, SuiteKind::Class),
        |f| fmt_class_header(f, *name, *arguments),
    )?;
    empty_lines_before_trailing_comments(f, comments.trailing(statement));
    Ok(())
}
