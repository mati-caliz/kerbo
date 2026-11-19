use super::clause::{ClauseComments, clause};
use super::suite::SuiteKind;
use crate::formatter::python::ast::{NodeId, Other, Span};
use crate::formatter::python::builders::{CommaSeparated, has_magic_trailing_comma, parenthesize_if_expands};
use crate::formatter::python::comments::SourceComment;
use crate::formatter::python::context::{Formatted, Formatter};
use crate::formatter::python::expression::can_omit::can_omit_optional_parentheses;
use crate::formatter::python::expression::{
    format_expr, format_expr_preserving, format_node_with, maybe_parenthesize_expression,
};
use crate::formatter::python::parentheses::{Parentheses, Parenthesize, optional_parentheses, parenthesized};

const PARENTHESES: (&str, &str) = ("(", ")");
const WITH_ITEM: &str = "ítem de with";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WithItemLayout {
    SingleParenthesizedContextManager,
    SingleWithoutTarget,
    ParenthesizedContextManagers { single: bool },
}

#[derive(Debug, Clone, Copy)]
enum WithItemsLayout {
    SingleParenthesizedContextManager(NodeId),
    SingleWithoutTarget(NodeId),
    SingleWithTarget(NodeId),
    ParenthesizeIfExpands,
    Parenthesized,
}

fn item_parts(f: &Formatter<'_>, item: NodeId) -> Formatted<(NodeId, Option<NodeId>)> {
    match f.ast.other(item) {
        Some(Other::WithItem {
            context_expr,
            optional_vars,
        }) => Ok((*context_expr, *optional_vars)),
        Some(_) | None => Err(f.unsupported(item, WITH_ITEM)),
    }
}

fn fmt_item_fields(f: &mut Formatter<'_>, item: NodeId, layout: WithItemLayout) -> Formatted {
    let (context_expr, optional_vars) = item_parts(f, item)?;
    let trailing_as_comments = f.comments.dangling(item);
    match layout {
        WithItemLayout::ParenthesizedContextManagers { single } => {
            let prefer_breaking_context_expression =
                (optional_vars.is_some() || !single) && f.is_expression_parenthesized(context_expr);
            if prefer_breaking_context_expression {
                maybe_parenthesize_expression(
                    f,
                    context_expr,
                    item,
                    Parenthesize::IfBreaksParenthesizedNested,
                )?;
            } else {
                format_expr(f, context_expr, Parentheses::Never)?;
            }
        }
        WithItemLayout::SingleParenthesizedContextManager | WithItemLayout::SingleWithoutTarget => {
            maybe_parenthesize_expression(f, context_expr, item, Parenthesize::IfBreaks)?;
        }
    }
    let Some(optional_vars) = optional_vars else {
        return Ok(());
    };
    f.space();
    f.token("as");
    f.space();
    if trailing_as_comments.is_empty() {
        return format_expr_preserving(f, optional_vars);
    }
    parenthesized(f, PARENTHESES, trailing_as_comments, |f| {
        format_expr(f, optional_vars, Parentheses::Never)
    })
}

fn fmt_item(f: &mut Formatter<'_>, item: NodeId, layout: WithItemLayout) -> Formatted {
    format_node_with(f, item, |f| fmt_item_fields(f, item, layout))
}

// Cómo se reparten los ítems: uno solo se queda sin paréntesis si su expresión ya puede partirse;
// varios, o una coma final, van entre paréntesis de a uno por línea si no entran.
fn layout_of(
    f: &Formatter<'_>,
    statement: NodeId,
    items: &[NodeId],
    parenthesized_comments: &[SourceComment],
) -> Formatted<WithItemsLayout> {
    if !parenthesized_comments.is_empty() {
        return Ok(WithItemsLayout::Parenthesized);
    }
    let after_last_item = items
        .last()
        .map(|last| Span::new(f.ast.end(*last), f.ast.end(statement)));
    if after_last_item.is_some_and(|span| has_magic_trailing_comma(f, span)) {
        return Ok(WithItemsLayout::ParenthesizeIfExpands);
    }
    let [single] = items else {
        return Ok(WithItemsLayout::ParenthesizeIfExpands);
    };
    let (context_expr, optional_vars) = item_parts(f, *single)?;
    if f.comments.has_leading(*single) || f.comments.has_trailing(*single) {
        return Ok(WithItemsLayout::Parenthesized);
    }
    Ok(if f.is_expression_parenthesized(context_expr) {
        WithItemsLayout::SingleParenthesizedContextManager(*single)
    } else if optional_vars.is_none() {
        WithItemsLayout::SingleWithoutTarget(*single)
    } else if can_omit_optional_parentheses(f, context_expr) {
        WithItemsLayout::SingleWithTarget(*single)
    } else {
        WithItemsLayout::ParenthesizeIfExpands
    })
}

fn fmt_joined_items(f: &mut Formatter<'_>, items: &[NodeId], sequence_end: usize) -> Formatted {
    let layout = WithItemLayout::ParenthesizedContextManagers {
        single: items.len() == 1,
    };
    let mut joiner = CommaSeparated::new(sequence_end);
    for item in items {
        joiner.entry(f, f.ast.end(*item), |f| fmt_item(f, *item, layout))?;
    }
    joiner.finish(f);
    Ok(())
}

fn fmt_items(
    f: &mut Formatter<'_>,
    statement: NodeId,
    (items, body): (&[NodeId], &[NodeId]),
    parenthesized_comments: &[SourceComment],
) -> Formatted {
    let sequence_end = body
        .first()
        .map_or_else(|| f.ast.end(statement), |first| f.ast.start(*first));
    match layout_of(f, statement, items, parenthesized_comments)? {
        WithItemsLayout::SingleWithTarget(single) => optional_parentheses(f, |f| {
            fmt_item(
                f,
                single,
                WithItemLayout::ParenthesizedContextManagers { single: true },
            )
        }),
        WithItemsLayout::SingleWithoutTarget(single) => {
            fmt_item(f, single, WithItemLayout::SingleWithoutTarget)
        }
        WithItemsLayout::SingleParenthesizedContextManager(single) => {
            fmt_item(f, single, WithItemLayout::SingleParenthesizedContextManager)
        }
        WithItemsLayout::ParenthesizeIfExpands => {
            parenthesize_if_expands(f, true, |f| fmt_joined_items(f, items, sequence_end))
        }
        WithItemsLayout::Parenthesized => parenthesized(f, PARENTHESES, parenthesized_comments, |f| {
            fmt_joined_items(f, items, sequence_end)
        }),
    }
}

pub fn fmt_with(
    f: &mut Formatter<'_>,
    statement: NodeId,
    is_async: bool,
    (items, body): (&[NodeId], &[NodeId]),
) -> Formatted {
    let dangling = f.comments.dangling(statement);
    let first_item_start = items.first().map(|first| f.ast.start(*first));
    let partition_point =
        dangling.partition_point(|comment| first_item_start.is_some_and(|start| start > comment.span.start));
    let (parenthesized_comments, colon_comments) = dangling.split_at(partition_point);
    clause(
        f,
        ClauseComments::trailing(colon_comments),
        (body, SuiteKind::Other),
        |f| {
            if is_async {
                f.token("async");
                f.space();
            }
            f.token("with");
            f.space();
            fmt_items(f, statement, (items, body), parenthesized_comments)
        },
    )
}
