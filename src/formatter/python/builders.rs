use super::ast::Span;
use super::context::{Formatted, Formatter, NodeLevel};
use super::trivia::{TokenKind, Tokenizer};
use crate::formatter::document::LineMode;

const COMMA: &str = ",";
const OPENING_PARENTHESIS: &str = "(";
const CLOSING_PARENTHESIS: &str = ")";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndentMode {
    Soft,
    Block,
}

// `block_indent` y sus variantes de ruff_formatter: el contenido va sangrado entre dos saltos, y
// si no escribe nada, no queda ni la sangría ni los saltos.
pub fn indented(
    f: &mut Formatter<'_>,
    mode: IndentMode,
    content: impl FnOnce(&mut Formatter<'_>) -> Formatted,
) -> Formatted {
    let before = f.len();
    f.start_indent();
    f.line(match mode {
        IndentMode::Soft => LineMode::Soft,
        IndentMode::Block => LineMode::Hard,
    });
    let before_content = f.len();
    content(f)?;
    if f.len() == before_content {
        f.truncate(before);
        return Ok(());
    }
    f.end_indent();
    match mode {
        IndentMode::Soft => f.line(LineMode::Soft),
        IndentMode::Block => f.line(LineMode::Hard),
    }
    Ok(())
}

pub fn soft_block_indent(
    f: &mut Formatter<'_>,
    content: impl FnOnce(&mut Formatter<'_>) -> Formatted,
) -> Formatted {
    indented(f, IndentMode::Soft, content)
}

pub fn block_indent(
    f: &mut Formatter<'_>,
    content: impl FnOnce(&mut Formatter<'_>) -> Formatted,
) -> Formatted {
    indented(f, IndentMode::Block, content)
}

pub fn group(f: &mut Formatter<'_>, content: impl FnOnce(&mut Formatter<'_>) -> Formatted) -> Formatted {
    f.start_group(None);
    content(f)?;
    f.end_group();
    Ok(())
}

// Los paréntesis sólo aparecen si el contenido no entra en la línea.
pub fn parenthesize_if_expands(
    f: &mut Formatter<'_>,
    indent: bool,
    content: impl FnOnce(&mut Formatter<'_>) -> Formatted,
) -> Formatted {
    let saved_level = f.enter(NodeLevel::ParenthesizedExpression);
    let parentheses = f.group_id();
    f.start_group(indent.then_some(parentheses));
    f.token_if_group_breaks(OPENING_PARENTHESIS);
    if indent {
        f.start_indent_if_group_breaks(parentheses);
        f.line(LineMode::Soft);
        content(f)?;
        f.end_indent_if_group_breaks();
        f.line(LineMode::Soft);
    } else {
        content(f)?;
    }
    f.token_if_group_breaks(CLOSING_PARENTHESIS);
    f.end_group();
    f.node_level = saved_level;
    Ok(())
}

// Una coma final en el fuente (la «coma mágica») deja la lista partida de a un elemento por línea.
pub fn has_magic_trailing_comma(f: &Formatter<'_>, span: Span) -> bool {
    Tokenizer::new(f.source, span)
        .skip_trivia()
        .find(|token| token.kind != TokenKind::RParen)
        .is_some_and(|token| token.kind == TokenKind::Comma)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrailingComma {
    MoreThanOne,
    OneOrMore,
}

// `join_comma_separated` de ruff: los elementos van separados por coma y salto blando, con coma
// final si la lista se parte.
#[derive(Debug)]
pub struct CommaSeparated {
    entries: usize,
    last_end: usize,
    sequence_end: usize,
    trailing_comma: TrailingComma,
}

impl CommaSeparated {
    pub const fn new(sequence_end: usize) -> Self {
        Self {
            entries: 0,
            last_end: 0,
            sequence_end,
            trailing_comma: TrailingComma::MoreThanOne,
        }
    }

    pub const fn with_trailing_comma(mut self, trailing_comma: TrailingComma) -> Self {
        self.trailing_comma = trailing_comma;
        self
    }

    pub fn entry(
        &mut self,
        f: &mut Formatter<'_>,
        entry_end: usize,
        content: impl FnOnce(&mut Formatter<'_>) -> Formatted,
    ) -> Formatted {
        self.entry_with_line_separator(f, (entry_end, LineMode::SoftOrSpace), content)
    }

    pub fn entry_with_line_separator(
        &mut self,
        f: &mut Formatter<'_>,
        (entry_end, separator): (usize, LineMode),
        content: impl FnOnce(&mut Formatter<'_>) -> Formatted,
    ) -> Formatted {
        if self.entries > 0 {
            f.token(COMMA);
            f.line(separator);
        }
        self.entries = self.entries.saturating_add(1);
        self.last_end = entry_end;
        content(f)
    }

    pub fn finish(self, f: &mut Formatter<'_>) {
        if self.entries == 0 {
            return;
        }
        let magic_trailing_comma = has_magic_trailing_comma(f, Span::new(self.last_end, self.sequence_end));
        if magic_trailing_comma || self.trailing_comma == TrailingComma::OneOrMore || self.entries > 1 {
            f.token_if_group_breaks(COMMA);
        }
        if magic_trailing_comma {
            f.expand_parent();
        }
    }
}
