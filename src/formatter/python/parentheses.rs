use super::builders::soft_block_indent;
use super::comments::SourceComment;
use super::comments::format::{dangling_comments, dangling_open_parenthesis_comments, trailing_comments};
use super::context::{Formatted, Formatter, NodeLevel};
use crate::formatter::document::{Condition, LineMode, PrintMode, TagKind};

const OPENING_PARENTHESIS: &str = "(";
const CLOSING_PARENTHESIS: &str = ")";

// Lo que una expresión pide de los paréntesis opcionales que la pueden rodear.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionalParentheses {
    Multiline,
    Always,
    BestFit,
    Never,
}

// Lo que el nodo padre deja hacer con esos paréntesis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Parenthesize {
    Optional,
    IfBreaks,
    IfRequired,
    IfBreaksParenthesized,
    IfBreaksParenthesizedNested,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Parentheses {
    Preserve,
    Always,
    Never,
}

// El contenido entre `left` y `right`, sangrado y de a una línea si no entra. Los comentarios son
// los que quedaron al final de la línea del paréntesis que abre.
pub fn parenthesized(
    f: &mut Formatter<'_>,
    (left, right): (&'static str, &'static str),
    comments: &[SourceComment],
    content: impl FnOnce(&mut Formatter<'_>) -> Formatted,
) -> Formatted {
    let enclosing_level = f.enter(NodeLevel::ParenthesizedExpression);
    f.token(left);
    let fits_expanded = if let NodeLevel::Expression(Some(group)) = enclosing_level {
        f.start_fits_expanded(Some(Condition::if_group_fits_on_line(group)));
        true
    } else {
        false
    };
    f.start_group(None);
    dangling_open_parenthesis_comments(f, comments);
    soft_block_indent(f, content)?;
    f.end_group();
    if fits_expanded {
        f.end(TagKind::FitsExpanded);
    }
    f.token(right);
    f.node_level = enclosing_level;
    Ok(())
}

// Como `parenthesized`, pero el contenido va pegado a los paréntesis, sin sangría ni grupo propio:
// así queda una cadena de varias líneas que es el único argumento de una llamada.
pub fn hugging_parentheses(
    f: &mut Formatter<'_>,
    (left, right): (&'static str, &'static str),
    content: impl FnOnce(&mut Formatter<'_>) -> Formatted,
) -> Formatted {
    let enclosing_level = f.enter(NodeLevel::ParenthesizedExpression);
    f.token(left);
    let fits_expanded = if let NodeLevel::Expression(Some(group)) = enclosing_level {
        f.start_fits_expanded(Some(Condition::if_group_fits_on_line(group)));
        true
    } else {
        false
    };
    content(f)?;
    if fits_expanded {
        f.end(TagKind::FitsExpanded);
    }
    f.token(right);
    f.node_level = enclosing_level;
    Ok(())
}

// Paréntesis que sólo aparecen si el contenido se parte, y que el contenido puede consultar por
// el grupo que abren (`NodeLevel::Expression`).
pub fn optional_parentheses(
    f: &mut Formatter<'_>,
    content: impl FnOnce(&mut Formatter<'_>) -> Formatted,
) -> Formatted {
    let parentheses = f.group_id();
    let saved_level = f.enter(NodeLevel::Expression(Some(parentheses)));
    f.start_group(Some(parentheses));
    f.token_if_group_breaks(OPENING_PARENTHESIS);
    f.start_indent_if_group_breaks(parentheses);
    f.line(LineMode::Soft);
    content(f)?;
    f.end_indent_if_group_breaks();
    f.line(LineMode::Soft);
    f.token_if_group_breaks(CLOSING_PARENTHESIS);
    f.end_group();
    f.node_level = saved_level;
    Ok(())
}

// Un salto blando que sólo vale adentro de paréntesis: afuera, Python no deja partir la línea.
pub fn in_parentheses_only_line_break(f: &mut Formatter<'_>, mode: LineMode) {
    let or_space = mode == LineMode::SoftOrSpace;
    match f.node_level {
        NodeLevel::TopLevel(_) | NodeLevel::CompoundStatement | NodeLevel::Expression(None) => {
            if or_space {
                f.space();
            }
        }
        NodeLevel::Expression(Some(parentheses)) => {
            f.start_conditional_content(PrintMode::Expanded, Some(parentheses));
            f.line(mode);
            f.end_conditional_content();
            if or_space {
                f.start_conditional_content(PrintMode::Flat, Some(parentheses));
                f.space();
                f.end_conditional_content();
            }
        }
        NodeLevel::ParenthesizedExpression => f.line(mode),
    }
}

pub fn in_parentheses_only_soft_line_break(f: &mut Formatter<'_>) {
    in_parentheses_only_line_break(f, LineMode::Soft);
}

pub fn in_parentheses_only_soft_line_break_or_space(f: &mut Formatter<'_>) {
    in_parentheses_only_line_break(f, LineMode::SoftOrSpace);
}

pub fn start_in_parentheses_only_group(f: &mut Formatter<'_>) {
    match f.node_level {
        NodeLevel::Expression(Some(parentheses)) => {
            f.start_conditional_group(Condition::if_group_breaks(parentheses));
        }
        NodeLevel::ParenthesizedExpression => f.start_group(None),
        NodeLevel::Expression(None) | NodeLevel::TopLevel(_) | NodeLevel::CompoundStatement => {}
    }
}

pub fn end_in_parentheses_only_group(f: &mut Formatter<'_>) {
    match f.node_level {
        NodeLevel::Expression(Some(_)) => f.end(TagKind::ConditionalGroup),
        NodeLevel::ParenthesizedExpression => f.end_group(),
        NodeLevel::Expression(None) | NodeLevel::TopLevel(_) | NodeLevel::CompoundStatement => {}
    }
}

// Un grupo que sólo existe adentro de paréntesis.
pub fn in_parentheses_only_group(
    f: &mut Formatter<'_>,
    content: impl FnOnce(&mut Formatter<'_>) -> Formatted,
) -> Formatted {
    start_in_parentheses_only_group(f);
    content(f)?;
    end_in_parentheses_only_group(f);
    Ok(())
}

pub fn in_parentheses_only_if_group_breaks(
    f: &mut Formatter<'_>,
    content: impl FnOnce(&mut Formatter<'_>) -> Formatted,
) -> Formatted {
    let group = match f.node_level {
        NodeLevel::TopLevel(_) | NodeLevel::CompoundStatement | NodeLevel::Expression(None) => {
            return Ok(());
        }
        NodeLevel::Expression(Some(parentheses)) => Some(parentheses),
        NodeLevel::ParenthesizedExpression => None,
    };
    f.start_conditional_content(PrintMode::Expanded, group);
    content(f)?;
    f.end_conditional_content();
    Ok(())
}

// Un par de paréntesis sin nada adentro salvo, quizás, comentarios.
pub fn empty_parenthesized(
    f: &mut Formatter<'_>,
    (left, right): (&'static str, &'static str),
    comments: &[SourceComment],
) -> Formatted {
    let end_of_line_split = comments.partition_point(|comment| !comment.is_own_line());
    let (end_of_line, own_line) = comments.split_at(end_of_line_split);
    f.start_group(None);
    f.token(left);
    trailing_comments(f, end_of_line);
    if !end_of_line.is_empty() {
        f.line(LineMode::Hard);
    }
    soft_block_indent(f, |f| {
        dangling_comments(f, own_line);
        Ok(())
    })?;
    f.token(right);
    f.end_group();
    Ok(())
}
