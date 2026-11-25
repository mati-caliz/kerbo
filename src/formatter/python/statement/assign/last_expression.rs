mod inlined_comments;

use std::rc::Rc;

use self::inlined_comments::InlinedComments;
use super::{AssignOperator, fmt_before_operator};
use crate::formatter::document::{BestFittingMode, Element, GroupId, PrintMode, Start, TagKind, will_break};
use crate::formatter::python::ast::{Expr, NodeId};
use crate::formatter::python::builders::{block_indent, parenthesize_if_expands, soft_block_indent};
use crate::formatter::python::context::{Formatted, Formatter, NodeLevel};
use crate::formatter::python::expression::lambda::{LambdaLayout, fmt_lambda};
use crate::formatter::python::expression::{
    format_expr, format_node_with, maybe_parenthesize_expression, needs_parentheses,
};
use crate::formatter::python::parentheses::{OptionalParentheses, Parentheses, Parenthesize};
use crate::formatter::python::string::implicit::{ImplicitLayout, fmt_expanded, fmt_flat, merged_flags};

const OPENING_PARENTHESIS: &str = "(";
const CLOSING_PARENTHESIS: &str = ")";

type Recorded = Rc<[Element]>;

fn indented_recording(f: &mut Formatter<'_>, recording: &Recorded) -> Formatted {
    soft_block_indent(f, |f| {
        f.interned(recording);
        Ok(())
    })
}

fn record(
    f: &mut Formatter<'_>,
    content: impl FnOnce(&mut Formatter<'_>) -> Formatted,
) -> Formatted<Recorded> {
    let start = f.len();
    content(f)?;
    Ok(f.take_since(start))
}

fn is_string_like(f: &Formatter<'_>, expression: NodeId) -> bool {
    matches!(
        f.ast.expr(expression),
        Some(Expr::StringLiteral { .. } | Expr::BytesLiteral { .. })
    )
}

fn should_inline_comments(f: &Formatter<'_>, expression: NodeId, parent: NodeId) -> bool {
    match f.ast.expr(expression) {
        Some(Expr::Name | Expr::NoneLiteral | Expr::NumberLiteral(_) | Expr::BooleanLiteral(_)) => true,
        Some(Expr::StringLiteral { .. } | Expr::BytesLiteral { .. }) => {
            needs_parentheses(f, expression, parent) == OptionalParentheses::BestFit
        }
        Some(_) | None => false,
    }
}

fn is_call_like(f: &Formatter<'_>, expression: NodeId) -> bool {
    matches!(
        f.ast.expr(expression),
        Some(Expr::Attribute { .. } | Expr::Call { .. } | Expr::Subscript { .. })
    )
}

fn maybe_parenthesize_value(f: &mut Formatter<'_>, value: NodeId, parent: NodeId) -> Formatted {
    let is_lambda = matches!(f.ast.expr(value), Some(Expr::Lambda { .. }));
    if is_lambda && !f.comments.has_leading(value) {
        return parenthesize_if_expands(f, true, |f| {
            format_node_with(f, value, |f| fmt_lambda(f, value, LambdaLayout::Assignment))
        });
    }
    maybe_parenthesize_expression(f, value, parent, Parenthesize::IfBreaks)
}

fn expanding_group(
    f: &mut Formatter<'_>,
    id: Option<GroupId>,
    content: impl FnOnce(&mut Formatter<'_>) -> Formatted,
) -> Formatted {
    f.start_group_expanding(id, true);
    content(f)?;
    f.end_group();
    Ok(())
}

fn record_expanded_string(f: &mut Formatter<'_>, value: NodeId, group: GroupId) -> Formatted<Recorded> {
    record(f, |f| {
        let saved_level = f.enter(NodeLevel::Expression(Some(group)));
        fmt_expanded(f, value, ImplicitLayout::MaybeFlat)?;
        f.node_level = saved_level;
        Ok(())
    })
}

// Una cadena pegada como valor: unida en la línea, unida entre paréntesis, o una parte por línea.
fn fmt_implicit_string_value(
    f: &mut Formatter<'_>,
    value: NodeId,
    flat: &Recorded,
    inline: InlinedComments<'_>,
) -> Formatted {
    let group = f.group_id();
    let expanded = record_expanded_string(f, value, group)?;
    let single_line = record(f, |f| {
        f.interned(flat);
        inline.write(f);
        Ok(())
    })?;
    let joined_parenthesized = record(f, |f| {
        expanding_group(f, Some(group), |f| {
            f.token(OPENING_PARENTHESIS);
            soft_block_indent(f, |f| {
                f.interned(flat);
                inline.write(f);
                Ok(())
            })?;
            f.token(CLOSING_PARENTHESIS);
            Ok(())
        })
    })?;
    let implicit_expanded = record(f, |f| {
        expanding_group(f, Some(group), |f| {
            f.token(OPENING_PARENTHESIS);
            block_indent(f, |f| {
                f.interned(&expanded);
                Ok(())
            })?;
            f.token(CLOSING_PARENTHESIS);
            inline.write(f);
            Ok(())
        })
    })?;
    f.best_fitting(
        vec![single_line, joined_parenthesized, implicit_expanded],
        BestFittingMode::AllLines,
    );
    Ok(())
}

fn fmt_best_fit_value(f: &mut Formatter<'_>, value: NodeId, inline: InlinedComments<'_>) -> Formatted {
    let group = f.group_id();
    f.start(Start::BestFitParenthesize { id: Some(group) });
    inline.mark_formatted();
    format_expr(f, value, Parentheses::Never)?;
    if !inline.is_empty() {
        f.start_conditional_content(PrintMode::Expanded, None);
        inline.write(f);
        f.end_conditional_content();
    }
    f.end(TagKind::BestFitParenthesize);
    if !inline.is_empty() {
        f.start_conditional_content(PrintMode::Flat, Some(group));
        inline.write(f);
        f.end_conditional_content();
    }
    Ok(())
}

fn record_flat_string(f: &mut Formatter<'_>, value: NodeId) -> Formatted<Option<Recorded>> {
    if !is_string_like(f, value) {
        return Ok(None);
    }
    let Some(flags) = merged_flags(f, value) else {
        return Ok(None);
    };
    record(f, |f| {
        fmt_flat(f, value, flags, false)?;
        Ok(())
    })
    .map(Some)
}

// El valor de un `return`, una asignación o una anotación, de izquierda a derecha: lo de antes ya
// se escribió y el valor gana paréntesis sólo si no entra en lo que queda de la línea.
pub fn left_to_right(f: &mut Formatter<'_>, value: NodeId, statement: NodeId) -> Formatted {
    let can_inline_comment = should_inline_comments(f, value, statement);
    let flat = record_flat_string(f, value)?;
    if !can_inline_comment && flat.is_none() {
        return maybe_parenthesize_value(f, value, statement);
    }
    let Some(inline) = InlinedComments::new(f, value, statement) else {
        return format_expr(f, value, Parentheses::Always);
    };
    match flat {
        Some(flat) => {
            inline.mark_formatted();
            fmt_implicit_string_value(f, value, &flat, inline)
        }
        None => fmt_best_fit_value(f, value, inline),
    }
}

fn should_non_inlineable_use_best_fit(f: &Formatter<'_>, expression: NodeId, parent: NodeId) -> bool {
    is_call_like(f, expression) && needs_parentheses(f, expression, parent) == OptionalParentheses::BestFit
}

#[derive(Debug)]
struct Parts<'comments> {
    last_target: Recorded,
    operator: AssignOperator,
    value: Recorded,
    inline: InlinedComments<'comments>,
}

impl Parts<'_> {
    fn target_and_operator(&self, f: &mut Formatter<'_>, split_target: bool) {
        if split_target {
            f.start_group_expanding(None, true);
            f.interned(&self.last_target);
            f.end_group();
        } else {
            f.interned(&self.last_target);
        }
        f.space();
        f.token(self.operator.as_str());
        f.space();
    }

    fn flat_value(&self, f: &mut Formatter<'_>, split_target: bool) -> Formatted<Recorded> {
        record(f, |f| {
            self.target_and_operator(f, split_target);
            f.interned(&self.value);
            self.inline.write(f);
            Ok(())
        })
    }

    fn indented_value(&self, f: &mut Formatter<'_>) -> Formatted {
        soft_block_indent(f, |f| {
            f.interned(&self.value);
            self.inline.write(f);
            Ok(())
        })
    }

    fn parenthesized_value(&self, f: &mut Formatter<'_>, split_target: bool) -> Formatted<Recorded> {
        record(f, |f| {
            self.target_and_operator(f, split_target);
            f.token(OPENING_PARENTHESIS);
            expanding_group(f, None, |f| self.indented_value(f))?;
            f.token(CLOSING_PARENTHESIS);
            Ok(())
        })
    }

    fn expanded_value(&self, f: &mut Formatter<'_>) -> Formatted<Recorded> {
        record(f, |f| {
            self.target_and_operator(f, false);
            f.start_group_expanding(None, true);
            f.interned(&self.value);
            f.end_group();
            Ok(())
        })
    }

    fn parenthesized_multiline(
        &self,
        f: &mut Formatter<'_>,
        split_target: bool,
        (expanded, group): (&Recorded, GroupId),
    ) -> Formatted<Recorded> {
        record(f, |f| {
            self.target_and_operator(f, split_target);
            f.token(OPENING_PARENTHESIS);
            expanding_group(f, Some(group), |f| indented_recording(f, expanded))?;
            f.token(CLOSING_PARENTHESIS);
            self.inline.write(f);
            Ok(())
        })
    }
}

fn fmt_string_variants(
    f: &mut Formatter<'_>,
    parts: &Parts<'_>,
    value: NodeId,
    last_target_breaks: bool,
) -> Formatted {
    let group = f.group_id();
    let expanded = record_expanded_string(f, value, group)?;
    let mut variants = Vec::new();
    if !last_target_breaks {
        variants.push(parts.flat_value(f, false)?);
        variants.push(parts.parenthesized_value(f, false)?);
        variants.push(parts.parenthesized_multiline(f, false, (&expanded, group))?);
    }
    variants.push(parts.flat_value(f, true)?);
    variants.push(parts.parenthesized_value(f, true)?);
    variants.push(parts.parenthesized_multiline(f, true, (&expanded, group))?);
    f.best_fitting(variants, BestFittingMode::AllLines);
    Ok(())
}

fn fmt_variants(f: &mut Formatter<'_>, parts: &Parts<'_>, value: NodeId, is_flat_string: bool) -> Formatted {
    if is_call_like(f, value) {
        let variants = vec![
            parts.flat_value(f, false)?,
            parts.expanded_value(f)?,
            parts.parenthesized_value(f, false)?,
            parts.flat_value(f, true)?,
        ];
        f.best_fitting(variants, BestFittingMode::FirstLine);
        return Ok(());
    }
    if is_flat_string {
        let last_target_breaks = will_break(&parts.last_target);
        return fmt_string_variants(f, parts, value, last_target_breaks);
    }
    let variants = vec![
        parts.flat_value(f, false)?,
        parts.parenthesized_value(f, false)?,
        parts.flat_value(f, true)?,
    ];
    f.best_fitting(variants, BestFittingMode::FirstLine);
    Ok(())
}

fn fmt_operator(f: &mut Formatter<'_>, before_operator: NodeId, operator: AssignOperator) -> Formatted {
    fmt_before_operator(f, before_operator)?;
    f.space();
    f.token(operator.as_str());
    f.space();
    Ok(())
}

// De derecha a izquierda: primero se intenta dejar entero lo que está antes del operador y partir
// el valor; el destino se parte sólo si no queda otra.
pub fn right_to_left(
    f: &mut Formatter<'_>,
    (before_operator, operator): (NodeId, AssignOperator),
    value: NodeId,
    statement: NodeId,
) -> Formatted {
    let should_inline = should_inline_comments(f, value, statement);
    let flat = record_flat_string(f, value)?;
    if !should_inline && flat.is_none() && !should_non_inlineable_use_best_fit(f, value, statement) {
        fmt_operator(f, before_operator, operator)?;
        return maybe_parenthesize_value(f, value, statement);
    }
    let comments = f.comments;
    let inline = if should_inline || flat.is_some() {
        InlinedComments::new(f, value, statement)
    } else if comments.has_leading(value) || comments.has_trailing_own_line(value) {
        None
    } else {
        Some(InlinedComments::default())
    };
    let Some(inline) = inline else {
        fmt_operator(f, before_operator, operator)?;
        return format_expr(f, value, Parentheses::Always);
    };
    inline.mark_formatted();
    let last_target = record(f, |f| fmt_before_operator(f, before_operator))?;
    if flat.is_none() && will_break(&last_target) {
        f.interned(&last_target);
        f.space();
        f.token(operator.as_str());
        f.space();
        format_expr(f, value, Parentheses::Never)?;
        inline.write(f);
        return Ok(());
    }
    let is_flat_string = flat.is_some();
    let value_content = match flat {
        Some(flat) => flat,
        None => record(f, |f| format_expr(f, value, Parentheses::Never))?,
    };
    let parts = Parts {
        last_target,
        operator,
        value: value_content,
        inline,
    };
    fmt_variants(f, &parts, value, is_flat_string)
}
