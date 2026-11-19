pub mod separators;

use crate::formatter::document::LineMode;
use crate::formatter::python::ast::{NodeId, Other, Span};
use crate::formatter::python::builders::{group, soft_block_indent};
use crate::formatter::python::comments::format::{
    dangling_comments, dangling_open_parenthesis_comments, leading_comments, leading_node_comments,
    trailing_comments,
};
use crate::formatter::python::comments::{LinePosition, SourceComment};
use crate::formatter::python::context::{Formatted, Formatter, NodeLevel};
use crate::formatter::python::expression::{format_expr_preserving, format_node_with};
use crate::formatter::python::parentheses::empty_parenthesized;
use crate::formatter::python::trivia::{TokenKind, Tokenizer};
use separators::{
    SeparatorCommentLocation, assign_argument_separator_comment_placement, find_parameter_separators,
};

const PARENTHESES: (&str, &str) = ("(", ")");
const COMMA: &str = ",";
const PARAMETERS: &str = "parámetros";
const PARAMETER_COMMENTS: &str = "comentarios sueltos entre parámetros";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParametersParentheses {
    Preserve,
    Never,
}

#[derive(Debug, Clone, Copy)]
struct Fields<'ast> {
    posonlyargs: &'ast [NodeId],
    args: &'ast [NodeId],
    vararg: Option<NodeId>,
    kwonlyargs: &'ast [NodeId],
    kwarg: Option<NodeId>,
    parenthesis_comments: &'ast [SourceComment],
    slash_comments: &'ast [SourceComment],
    star_comments: &'ast [SourceComment],
}

impl Fields<'_> {
    const fn len(&self) -> usize {
        self.posonlyargs
            .len()
            .saturating_add(self.args.len())
            .saturating_add(self.kwonlyargs.len())
            .saturating_add(if self.vararg.is_some() { 1 } else { 0 })
            .saturating_add(if self.kwarg.is_some() { 1 } else { 0 })
    }

    const fn ends_with_positional_only_separator(&self) -> bool {
        !self.posonlyargs.is_empty()
            && self.args.is_empty()
            && self.vararg.is_none()
            && self.kwonlyargs.is_empty()
            && self.kwarg.is_none()
    }
}

const fn is_opening_bracket(kind: TokenKind) -> bool {
    matches!(kind, TokenKind::LParen | TokenKind::LBrace | TokenKind::LBracket)
}

// El primer comentario suelto puede ser el de la línea del paréntesis que abre: `def f(  # así`.
fn follows_the_opening_parenthesis(f: &Formatter<'_>, parameters: NodeId, comment: &SourceComment) -> bool {
    let before = Span::new(f.ast.start(parameters), comment.span.start);
    comment.line_position == LinePosition::EndOfLine
        && Tokenizer::new(f.source, before)
            .skip_trivia()
            .all(|token| is_opening_bracket(token.kind))
}

// Los comentarios sueltos de los parámetros, repartidos: el del paréntesis que abre, los del `/`
// y los del `*`.
fn split_dangling<'ast>(
    f: &Formatter<'ast>,
    parameters: NodeId,
    has_positional_only: bool,
) -> Formatted<[&'ast [SourceComment]; 3]> {
    let dangling = f.comments.dangling(parameters);
    let parenthesis_end = usize::from(
        dangling
            .first()
            .is_some_and(|comment| follows_the_opening_parenthesis(f, parameters, comment)),
    );
    let (parenthesis, separators) = dangling.split_at(parenthesis_end);
    let (slash, star) = find_parameter_separators(f.ast, f.source, parameters);
    let location_of = |comment: &SourceComment| {
        assign_argument_separator_comment_placement(
            slash.as_ref(),
            star.as_ref(),
            (comment.span, comment.line_position),
        )
    };
    if separators.iter().any(|comment| location_of(comment).is_none()) {
        return Err(f.unsupported(parameters, PARAMETER_COMMENTS));
    }
    let slash_end = if has_positional_only {
        separators.partition_point(|comment| {
            location_of(comment).is_some_and(SeparatorCommentLocation::is_on_the_slash)
        })
    } else {
        0
    };
    let (on_the_slash, on_the_star) = separators.split_at(slash_end);
    Ok([parenthesis, on_the_slash, on_the_star])
}

fn fields_of<'ast>(f: &Formatter<'ast>, parameters: NodeId) -> Formatted<Fields<'ast>> {
    let Some(Other::Parameters {
        posonlyargs,
        args,
        vararg,
        kwonlyargs,
        kwarg,
    }) = f.ast.other(parameters)
    else {
        return Err(f.unsupported(parameters, PARAMETERS));
    };
    let [parenthesis_comments, slash_comments, star_comments] =
        split_dangling(f, parameters, !posonlyargs.is_empty())?;
    Ok(Fields {
        posonlyargs,
        args,
        vararg: *vararg,
        kwonlyargs,
        kwarg: *kwarg,
        parenthesis_comments,
        slash_comments,
        star_comments,
    })
}

// `CommentsAroundText` de ruff: el `/` o el `*` con sus comentarios, los de línea propia arriba.
fn fmt_separator(f: &mut Formatter<'_>, text: &'static str, comments: &[SourceComment]) {
    let own_line_end = comments.partition_point(SourceComment::is_own_line);
    let (leading, trailing) = comments.split_at(own_line_end);
    leading_comments(f, leading);
    f.token(text);
    trailing_comments(f, trailing);
}

pub fn fmt_parameter(f: &mut Formatter<'_>, parameter: NodeId) -> Formatted {
    let Some(Other::Parameter { name, annotation }) = f.ast.other(parameter) else {
        return Err(f.unsupported(parameter, PARAMETERS));
    };
    format_node_with(f, parameter, |f| {
        f.source_text(*name);
        let Some(annotation) = annotation else {
            return Ok(());
        };
        f.token(":");
        if f.comments.has_leading(*annotation) && !f.is_expression_parenthesized(*annotation) {
            f.line(LineMode::Hard);
        } else {
            f.space();
        }
        format_expr_preserving(f, *annotation)
    })
}

// Un comentario entre el `=` y un valor por defecto sin paréntesis obliga a bajar el valor.
fn default_needs_line_break(f: &Formatter<'_>, parameter: NodeId, default: NodeId) -> bool {
    if f.comments.has_trailing(parameter) {
        return true;
    }
    f.comments.leading(default).first().is_some_and(|comment| {
        let between = Span::new(f.ast.end(parameter), comment.span.start);
        Tokenizer::new(f.source, between)
            .skip_trivia()
            .skip_while(|token| token.kind == TokenKind::RParen)
            .nth(1)
            .is_none()
    })
}

fn fmt_parameter_with_default(f: &mut Formatter<'_>, node: NodeId) -> Formatted {
    let Some(Other::ParameterWithDefault { parameter, default }) = f.ast.other(node) else {
        return Err(f.unsupported(node, PARAMETERS));
    };
    format_node_with(f, node, |f| {
        fmt_parameter(f, *parameter)?;
        let Some(default) = default else {
            return Ok(());
        };
        let has_annotation = matches!(
            f.ast.other(*parameter),
            Some(Other::Parameter {
                annotation: Some(_),
                ..
            })
        );
        let needs_line_break = default_needs_line_break(f, *parameter, *default);
        if has_annotation {
            f.space();
        }
        f.token("=");
        if needs_line_break {
            f.line(LineMode::Hard);
        } else if has_annotation {
            f.space();
        }
        format_expr_preserving(f, *default)
    })
}

#[derive(Debug)]
struct Joiner {
    is_first: bool,
}

impl Joiner {
    fn separate(&mut self, f: &mut Formatter<'_>) {
        if !self.is_first {
            f.token(COMMA);
            if f.node_level.is_parenthesized() {
                f.line(LineMode::SoftOrSpace);
            } else {
                f.space();
            }
        }
        self.is_first = false;
    }
}

fn fmt_variadic(f: &mut Formatter<'_>, parameter: NodeId, stars: &'static str) -> Formatted {
    leading_node_comments(f, parameter);
    f.token(stars);
    fmt_parameter(f, parameter)
}

// Los parámetros separados por comas, con `/` después de los sólo posicionales y `*` antes de los
// sólo por nombre. Devuelve el último, para buscar la coma final.
fn fmt_joined(f: &mut Formatter<'_>, fields: &Fields<'_>) -> Formatted<Option<NodeId>> {
    let mut joiner = Joiner { is_first: true };
    let mut last_node = None;
    for parameter in fields.posonlyargs {
        joiner.separate(f);
        fmt_parameter_with_default(f, *parameter)?;
        last_node = Some(*parameter);
    }
    if !fields.posonlyargs.is_empty() {
        joiner.separate(f);
        fmt_separator(f, "/", fields.slash_comments);
    }
    for parameter in fields.args {
        joiner.separate(f);
        fmt_parameter_with_default(f, *parameter)?;
        last_node = Some(*parameter);
    }
    if let Some(vararg) = fields.vararg {
        joiner.separate(f);
        fmt_variadic(f, vararg, "*")?;
        last_node = Some(vararg);
    } else if !fields.kwonlyargs.is_empty() {
        joiner.separate(f);
        fmt_separator(f, "*", fields.star_comments);
    }
    for parameter in fields.kwonlyargs {
        joiner.separate(f);
        fmt_parameter_with_default(f, *parameter)?;
        last_node = Some(*parameter);
    }
    if let Some(kwarg) = fields.kwarg {
        joiner.separate(f);
        fmt_variadic(f, kwarg, "**")?;
        last_node = Some(kwarg);
    }
    Ok(last_node)
}

fn has_trailing_comma(f: &Formatter<'_>, fields: &Fields<'_>, last_node: Option<NodeId>) -> bool {
    let Some(last_node) = last_node else {
        return false;
    };
    let separator_tokens = if fields.ends_with_positional_only_separator() {
        2
    } else {
        0
    };
    Tokenizer::starts_at(f.ast.end(last_node), f.source)
        .skip_trivia()
        .nth(separator_tokens)
        .is_some_and(|token| token.kind == TokenKind::Comma)
}

fn fmt_inner(f: &mut Formatter<'_>, fields: &Fields<'_>, parentheses: ParametersParentheses) -> Formatted {
    let last_node = fmt_joined(f, fields)?;
    let trailing_comma = has_trailing_comma(f, fields, last_node);
    if parentheses == ParametersParentheses::Never {
        if trailing_comma {
            f.token(COMMA);
        }
        return Ok(());
    }
    f.token_if_group_breaks(COMMA);
    if trailing_comma {
        f.line(LineMode::Hard);
    }
    Ok(())
}

pub fn fmt_parameters(
    f: &mut Formatter<'_>,
    parameters: NodeId,
    parentheses: ParametersParentheses,
) -> Formatted {
    let fields = fields_of(f, parameters)?;
    let dangling = f.comments.dangling(parameters);
    let count = fields.len();
    if parentheses == ParametersParentheses::Never {
        group(f, |f| fmt_inner(f, &fields, parentheses))?;
        dangling_comments(f, dangling);
        return Ok(());
    }
    let saved_level = f.enter(NodeLevel::ParenthesizedExpression);
    if count == 0 {
        empty_parenthesized(f, PARENTHESES, dangling)?;
    } else {
        let is_single = count == 1 && fields.posonlyargs.is_empty() && fields.kwonlyargs.is_empty();
        f.token(PARENTHESES.0);
        dangling_open_parenthesis_comments(f, fields.parenthesis_comments);
        soft_block_indent(f, |f| {
            if is_single {
                fmt_inner(f, &fields, parentheses)
            } else {
                group(f, |f| fmt_inner(f, &fields, parentheses))
            }
        })?;
        f.token(PARENTHESES.1);
    }
    f.node_level = saved_level;
    Ok(())
}

pub fn parameters_are_empty(f: &Formatter<'_>, parameters: NodeId) -> bool {
    matches!(
        f.ast.other(parameters),
        Some(Other::Parameters {
            posonlyargs,
            args,
            vararg: None,
            kwonlyargs,
            kwarg: None,
        }) if posonlyargs.is_empty() && args.is_empty() && kwonlyargs.is_empty()
    )
}
