use crate::formatter::python::ast::{Expr, NodeId, Other, Span};
use crate::formatter::python::builders::{CommaSeparated, group, has_magic_trailing_comma};
use crate::formatter::python::context::{Formatted, Formatter};
use crate::formatter::python::expression::comprehension::{GeneratorParentheses, fmt_generator};
use crate::formatter::python::expression::{format_expr, format_expr_preserving, format_node_with};
use crate::formatter::python::parentheses::{
    Parentheses, empty_parenthesized, hugging_parentheses, parenthesized,
};
use crate::formatter::python::string;
use crate::formatter::python::trivia::{TokenKind, Tokenizer, is_line_break, is_python_whitespace};

const PARENTHESES: (&str, &str) = ("(", ")");

// `f((a))`: el único argumento conserva sus paréntesis si después de él hay dos que cierran.
fn is_single_argument_parenthesized(f: &Formatter<'_>, argument: NodeId, call_end: usize) -> bool {
    let mut has_seen_right_parenthesis = false;
    for token in Tokenizer::new(f.source, Span::new(f.ast.end(argument), call_end)).skip_trivia() {
        if token.kind == TokenKind::RParen {
            if has_seen_right_parenthesis {
                return true;
            }
            has_seen_right_parenthesis = true;
        } else if token.kind != TokenKind::Comma {
            break;
        }
    }
    false
}

fn fmt_keyword(f: &mut Formatter<'_>, keyword: NodeId) -> Formatted {
    let Some(Other::Keyword { arg, value }) = f.ast.other(keyword) else {
        return Err(f.unsupported(keyword, "argumento"));
    };
    format_node_with(f, keyword, |f| {
        if let Some(arg) = arg {
            f.source_text(*arg);
            f.token("=");
        } else {
            f.token("**");
        }
        format_expr_preserving(f, *value)
    })
}

fn fmt_single_argument(f: &mut Formatter<'_>, argument: NodeId, call_end: usize) -> Formatted {
    if matches!(f.ast.expr(argument), Some(Expr::Generator { .. })) {
        return format_node_with(f, argument, |f| {
            fmt_generator(f, argument, GeneratorParentheses::Preserve)
        });
    }
    let parentheses = if is_single_argument_parenthesized(f, argument, call_end) {
        Parentheses::Always
    } else {
        Parentheses::Never
    };
    format_expr(f, argument, parentheses)
}

fn fmt_all_arguments(f: &mut Formatter<'_>, arguments: NodeId, single: Option<NodeId>) -> Formatted {
    let call_end = f.ast.end(arguments);
    let mut joiner = CommaSeparated::new(call_end);
    if let Some(argument) = single {
        joiner.entry(f, f.ast.end(argument), |f| {
            fmt_single_argument(f, argument, call_end)
        })?;
    } else {
        for argument in f.ast.children(arguments) {
            joiner.entry(f, f.ast.end(argument), |f| fmt_argument(f, argument))?;
        }
    }
    joiner.finish(f);
    Ok(())
}

// Una cadena de varias líneas con comillas triples que empieza en la línea del paréntesis.
fn is_huggable_string_argument(f: &Formatter<'_>, argument: NodeId, arguments: NodeId) -> bool {
    if !string::is_single_multiline_triple_quoted(f, argument) {
        return false;
    }
    let after_parenthesis = f.ast.start(arguments).saturating_add(PARENTHESES.0.len());
    let between_parentheses = f.text_of(Span::new(after_parenthesis, f.ast.start(argument)));
    !between_parentheses
        .trim_end_matches(is_python_whitespace)
        .ends_with(is_line_break)
}

// El argumento que puede ir pegado a los paréntesis de la llamada, sin sangría propia.
fn huggable_argument(f: &Formatter<'_>, arguments: NodeId, data: (&[NodeId], &[NodeId])) -> Option<NodeId> {
    let argument = match data {
        ([argument], []) => *argument,
        ([], [keyword]) => match f.ast.other(*keyword) {
            Some(Other::Keyword { arg: None, value }) if !f.comments.has(*keyword) => *value,
            Some(_) | None => return None,
        },
        _ => return None,
    };
    let comments = f.comments;
    let is_huggable = is_huggable_string_argument(f, argument, arguments)
        && !comments.has_leading(argument)
        && !comments.has_trailing(argument)
        && !has_magic_trailing_comma(f, Span::new(f.ast.end(argument), f.ast.end(arguments)));
    is_huggable.then_some(argument)
}

fn fmt_argument(f: &mut Formatter<'_>, argument: NodeId) -> Formatted {
    if f.ast.other(argument).is_some() {
        fmt_keyword(f, argument)
    } else {
        format_expr_preserving(f, argument)
    }
}

pub fn fmt_arguments(f: &mut Formatter<'_>, arguments: NodeId) -> Formatted {
    let Some(Other::Arguments { args, keywords }) = f.ast.other(arguments) else {
        return Err(f.unsupported(arguments, "argumentos"));
    };
    let dangling = f.comments.dangling(arguments);
    if args.is_empty() && keywords.is_empty() {
        return empty_parenthesized(f, PARENTHESES, dangling);
    }
    let single = match (args.as_slice(), keywords.as_slice()) {
        ([argument], []) => Some(*argument),
        _ => None,
    };
    let content = |formatter: &mut Formatter<'_>| {
        group(formatter, |nested| fmt_all_arguments(nested, arguments, single))
    };
    if dangling.is_empty() && huggable_argument(f, arguments, (args, keywords)).is_some() {
        hugging_parentheses(f, PARENTHESES, content)
    } else {
        parenthesized(f, PARENTHESES, dangling, content)
    }
}
