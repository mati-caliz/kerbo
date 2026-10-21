use std::collections::HashSet;
use std::sync::OnceLock;

use crate::deploy::sql::lexer::{Token, TokenKind};

const NON_VOLATILE_FUNCTIONS: &str = include_str!("non_volatile_functions.txt");
const HEADER_MARK: char = '#';
const OPEN_PARENTHESIS: char = '(';
const CLOSE_PARENTHESIS: char = ')';
const OPEN_BRACKET: char = '[';
const CLOSE_BRACKET: char = ']';
const CAST_MARK: char = ':';
const QUALIFIER_SEPARATOR: char = '.';
const OPERATOR_SYMBOLS: &[char] = &[
    '+', '-', '*', '/', '%', '^', '<', '>', '=', '|', '&', '!', '~', '#', '@',
];
const OPERATOR_WORDS: &[&str] = &["and", "or"];
const CONSTANT_WORDS: &[&str] = &["true", "false", "null"];
const CURRENT_TIMESTAMP: &str = "current_timestamp";
const ARRAY_WORD: &str = "array";
const CAST_WORD: &str = "cast";
const CAST_SEPARATOR: &str = "as";
const COLLATE_WORD: &str = "collate";

fn non_volatile_functions() -> &'static HashSet<&'static str> {
    static FUNCTIONS: OnceLock<HashSet<&'static str>> = OnceLock::new();
    FUNCTIONS.get_or_init(|| {
        NON_VOLATILE_FUNCTIONS
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with(HEADER_MARK))
            .collect()
    })
}

pub fn is_non_volatile(tokens: &[Token]) -> bool {
    if let Some(before) = before_top_level_word(tokens, COLLATE_WORD) {
        return is_non_volatile(before);
    }
    if let Some(split) = binary_split(tokens) {
        let (left, right) = tokens.split_at(split);
        let right_start = right
            .iter()
            .position(|token| !is_operator(token))
            .unwrap_or(right.len());
        return is_non_volatile(left) && is_non_volatile(right.get(right_start..).unwrap_or_default());
    }
    if let Some(cast) = cast_split(tokens) {
        return is_non_volatile(tokens.get(..cast).unwrap_or_default());
    }
    term(tokens)
}

fn term(tokens: &[Token]) -> bool {
    match tokens {
        [single] => match &single.kind {
            TokenKind::Literal => true,
            TokenKind::Word(word) => CONSTANT_WORDS.contains(&word.as_str()) || word == CURRENT_TIMESTAMP,
            TokenKind::Quoted(_) | TokenKind::Symbol(_) => false,
        },
        [first, second, ..] if first.is_word(ARRAY_WORD) && second.is_symbol(OPEN_BRACKET) => true,
        [first, second, ..] if first.is_word(CAST_WORD) && second.is_symbol(OPEN_PARENTHESIS) => {
            let inside = tokens.get(2..tokens.len().saturating_sub(1)).unwrap_or_default();
            before_top_level_word(inside, CAST_SEPARATOR).is_some_and(is_non_volatile)
        }
        [name, open, close] if open.is_symbol(OPEN_PARENTHESIS) && close.is_symbol(CLOSE_PARENTHESIS) => name
            .word()
            .is_some_and(|word| non_volatile_functions().contains(word)),
        [typ @ .., literal] if !typ.is_empty() => {
            literal.kind == TokenKind::Literal && typ.iter().all(|word| word.word().is_some())
        }
        _ => false,
    }
}

fn before_top_level_word<'tokens>(tokens: &'tokens [Token], word: &str) -> Option<&'tokens [Token]> {
    let index = top_level(tokens).find(|(_, token)| token.is_word(word))?.0;
    tokens.get(..index)
}

fn binary_split(tokens: &[Token]) -> Option<usize> {
    top_level(tokens)
        .find(|(index, token)| {
            *index > 0
                && is_operator(token)
                && tokens
                    .get(index.saturating_sub(1))
                    .is_some_and(|before| !is_operator(before))
        })
        .map(|(index, _)| index)
}

fn cast_split(tokens: &[Token]) -> Option<usize> {
    top_level(tokens)
        .find(|(index, token)| {
            token.is_symbol(CAST_MARK)
                && tokens
                    .get(index.saturating_add(1))
                    .is_some_and(|next| next.is_symbol(CAST_MARK))
        })
        .map(|(index, _)| index)
}

fn is_operator(token: &Token) -> bool {
    match &token.kind {
        TokenKind::Symbol(symbol) => OPERATOR_SYMBOLS.contains(symbol),
        TokenKind::Word(word) => OPERATOR_WORDS.contains(&word.as_str()),
        TokenKind::Quoted(_) | TokenKind::Literal => false,
    }
}

fn top_level(tokens: &[Token]) -> impl Iterator<Item = (usize, &Token)> {
    let mut depth = 0_usize;
    tokens.iter().enumerate().filter(move |(_, token)| {
        let at_top = depth == 0;
        if token.is_symbol(OPEN_PARENTHESIS) || token.is_symbol(OPEN_BRACKET) {
            depth = depth.saturating_add(1);
        } else if token.is_symbol(CLOSE_PARENTHESIS) || token.is_symbol(CLOSE_BRACKET) {
            depth = depth.saturating_sub(1);
        }
        at_top && !token.is_symbol(QUALIFIER_SEPARATOR)
    })
}
