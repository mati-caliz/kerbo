mod arguments;
mod omit;
mod sequence;

use super::ast::{NodeId, Other, Singleton};
use super::builders::parenthesize_if_expands;
use super::comments::SourceComment;
use super::comments::format::{dangling_comments, leading_comments};
use super::context::{Formatted, Formatter, NodeLevel};
use super::expression::{format_expr, format_expr_preserving, format_node_with, needs_parentheses};
use super::parentheses::{
    OptionalParentheses, Parentheses, in_parentheses_only_group,
    in_parentheses_only_soft_line_break_or_space, optional_parentheses, parenthesized,
};
use super::trivia::{BackwardsTokenizer, TokenKind, Tokenizer};
use crate::formatter::document::{LineMode, Start, TagKind};

const OPENING_PARENTHESIS: &str = "(";
const CLOSING_PARENTHESIS: &str = ")";
const PATTERN: &str = "patrón";
const WILDCARD: &str = "_";
const AS: &str = "as";
const UNION: &str = "|";
const STAR: &str = "*";

pub fn is_pattern_parenthesized(f: &Formatter<'_>, pattern: NodeId) -> bool {
    let span = f.ast.span(pattern);
    let closes = Tokenizer::starts_at(span.end, f.source)
        .skip_trivia()
        .next()
        .is_some_and(|token| token.kind == TokenKind::RParen);
    closes
        && BackwardsTokenizer::up_to(span.start, f.source, f.comment_ranges)
            .skip_trivia()
            .next()
            .is_some_and(|token| token.kind == TokenKind::LParen)
}

fn fmt_as(f: &mut Formatter<'_>, node: NodeId) -> Formatted {
    let Some(Other::PatternAs { pattern, name }) = f.ast.other(node) else {
        return Err(f.unsupported(node, PATTERN));
    };
    let Some(name) = name else {
        f.token(WILDCARD);
        return Ok(());
    };
    if let Some(pattern) = pattern {
        fmt_pattern(f, *pattern, Parentheses::Preserve)?;
        if f.comments.has_trailing(*pattern) {
            f.line(LineMode::Hard);
        } else {
            f.space();
        }
        f.token(AS);
        let trailing_as_comments = f.comments.dangling(node);
        if trailing_as_comments.is_empty() {
            f.space();
        } else if trailing_as_comments.iter().all(SourceComment::is_own_line) {
            f.line(LineMode::Hard);
        }
        dangling_comments(f, trailing_as_comments);
    }
    f.source_text(*name);
    Ok(())
}

fn fmt_or(f: &mut Formatter<'_>, node: NodeId) -> Formatted {
    let Some(Other::PatternOr { patterns }) = f.ast.other(node) else {
        return Err(f.unsupported(node, PATTERN));
    };
    in_parentheses_only_group(f, |f| {
        let Some((first, rest)) = patterns.split_first() else {
            return Ok(());
        };
        fmt_pattern(f, *first, Parentheses::Preserve)?;
        for pattern in rest {
            let leading = f.comments.leading(*pattern);
            if leading.is_empty() {
                in_parentheses_only_soft_line_break_or_space(f);
            } else {
                f.line(LineMode::Hard);
                leading_comments(f, leading);
            }
            f.token(UNION);
            f.space();
            fmt_pattern(f, *pattern, Parentheses::Preserve)?;
        }
        Ok(())
    })
}

fn fmt_fields(f: &mut Formatter<'_>, node: NodeId) -> Formatted {
    match f.ast.other(node) {
        Some(Other::PatternValue { value }) => format_expr(f, *value, Parentheses::Never),
        Some(Other::PatternSingleton(singleton)) => {
            f.token(match singleton {
                Singleton::None => "None",
                Singleton::True => "True",
                Singleton::False => "False",
            });
            Ok(())
        }
        Some(Other::PatternSequence { .. }) => sequence::fmt_sequence(f, node),
        Some(Other::PatternMapping { .. }) => sequence::fmt_mapping(f, node),
        Some(Other::PatternClass { cls, arguments }) => {
            format_expr_preserving(f, *cls)?;
            dangling_comments(f, f.comments.dangling(node));
            arguments::fmt_arguments(f, *arguments)
        }
        Some(Other::PatternStar { name }) => {
            f.token(STAR);
            dangling_comments(f, f.comments.dangling(node));
            match name {
                Some(name) => f.source_text(*name),
                None => f.token(WILDCARD),
            }
            Ok(())
        }
        Some(Other::PatternAs { .. }) => fmt_as(f, node),
        Some(Other::PatternOr { .. }) => fmt_or(f, node),
        Some(_) | None => Err(f.unsupported(node, PATTERN)),
    }
}

fn fmt_node(f: &mut Formatter<'_>, node: NodeId) -> Formatted {
    format_node_with(f, node, |f| fmt_fields(f, node))
}

pub fn fmt_pattern(f: &mut Formatter<'_>, node: NodeId, parentheses: Parentheses) -> Formatted {
    let parenthesize = match parentheses {
        Parentheses::Preserve => is_pattern_parenthesized(f, node),
        Parentheses::Always => true,
        Parentheses::Never => false,
    };
    if !parenthesize {
        return fmt_node(f, node);
    }
    let open_parenthesis_comment = f
        .comments
        .leading(node)
        .first()
        .filter(|comment| !comment.is_own_line())
        .map(std::slice::from_ref)
        .unwrap_or_default();
    parenthesized(
        f,
        (OPENING_PARENTHESIS, CLOSING_PARENTHESIS),
        open_parenthesis_comment,
        |nested| fmt_node(nested, node),
    )
}

fn pattern_needs_parentheses(f: &Formatter<'_>, node: NodeId, case: NodeId) -> OptionalParentheses {
    match f.ast.other(node) {
        Some(Other::PatternValue { value }) => needs_parentheses(f, *value, case),
        Some(Other::PatternSingleton(_)) => OptionalParentheses::BestFit,
        Some(Other::PatternAs { name, .. }) => {
            if name.is_some() {
                OptionalParentheses::Multiline
            } else {
                OptionalParentheses::BestFit
            }
        }
        Some(Other::PatternOr { .. }) => OptionalParentheses::Multiline,
        Some(Other::PatternClass { .. }) if f.comments.has_dangling(node) => OptionalParentheses::Always,
        Some(_) | None => OptionalParentheses::Never,
    }
}

fn fmt_best_fit_pattern(f: &mut Formatter<'_>, pattern: NodeId) -> Formatted {
    let group = f.group_id();
    let saved_level = f.enter(NodeLevel::Expression(Some(group)));
    f.start(Start::BestFitParenthesize { id: Some(group) });
    fmt_pattern(f, pattern, Parentheses::Never)?;
    f.end(TagKind::BestFitParenthesize);
    f.node_level = saved_level;
    Ok(())
}

pub fn maybe_parenthesize_pattern(f: &mut Formatter<'_>, pattern: NodeId, case: NodeId) -> Formatted {
    let comments = f.comments;
    if comments.has_leading(pattern) || comments.has_trailing_own_line(pattern) {
        return fmt_pattern(f, pattern, Parentheses::Always);
    }
    match pattern_needs_parentheses(f, pattern, case) {
        OptionalParentheses::Always => fmt_pattern(f, pattern, Parentheses::Always),
        OptionalParentheses::Never => fmt_pattern(f, pattern, Parentheses::Never),
        OptionalParentheses::Multiline => {
            if omit::can_omit_optional_parentheses(f, pattern) {
                optional_parentheses(f, |f| fmt_pattern(f, pattern, Parentheses::Never))
            } else {
                parenthesize_if_expands(f, true, |f| fmt_pattern(f, pattern, Parentheses::Never))
            }
        }
        OptionalParentheses::BestFit => {
            if comments.has_trailing(pattern) {
                fmt_pattern(f, pattern, Parentheses::Always)
            } else {
                fmt_best_fit_pattern(f, pattern)
            }
        }
    }
}
