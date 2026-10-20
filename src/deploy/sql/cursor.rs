use super::lexer::Token;

const STATEMENT_END: char = ';';
const LIST_SEPARATOR: char = ',';
const OPEN_PARENTHESIS: char = '(';
const CLOSE_PARENTHESIS: char = ')';
const QUALIFIER_SEPARATOR: char = '.';

pub fn statements(tokens: &[Token]) -> Vec<&[Token]> {
    tokens
        .split(|token| token.is_symbol(STATEMENT_END))
        .filter(|statement| !statement.is_empty())
        .collect()
}

pub fn top_level_items(tokens: &[Token]) -> Vec<&[Token]> {
    let mut items = Vec::new();
    let mut depth = 0_usize;
    let mut start = 0_usize;
    for (index, token) in tokens.iter().enumerate() {
        if token.is_symbol(OPEN_PARENTHESIS) {
            depth = depth.saturating_add(1);
        } else if token.is_symbol(CLOSE_PARENTHESIS) {
            depth = depth.saturating_sub(1);
        } else if depth == 0 && token.is_symbol(LIST_SEPARATOR) {
            items.extend(tokens.get(start..index));
            start = index.saturating_add(1);
        }
    }
    items.extend(tokens.get(start..));
    items.retain(|item| !item.is_empty());
    items
}

pub fn top_level_words(tokens: &[Token]) -> Vec<&str> {
    let mut depth = 0_usize;
    let mut words = Vec::new();
    for token in tokens {
        if token.is_symbol(OPEN_PARENTHESIS) {
            depth = depth.saturating_add(1);
        } else if token.is_symbol(CLOSE_PARENTHESIS) {
            depth = depth.saturating_sub(1);
        } else if depth == 0 {
            words.extend(token.word());
        }
    }
    words
}

#[derive(Debug, Clone, Copy)]
pub struct Cursor<'tokens> {
    tokens: &'tokens [Token],
}

impl<'tokens> Cursor<'tokens> {
    pub const fn new(tokens: &'tokens [Token]) -> Self {
        Self { tokens }
    }

    pub const fn peek(&self) -> Option<&'tokens Token> {
        self.tokens.first()
    }

    pub const fn rest(&self) -> &'tokens [Token] {
        self.tokens
    }

    pub fn next_token(&mut self) -> Option<&'tokens Token> {
        let (first, rest) = self.tokens.split_first()?;
        self.tokens = rest;
        Some(first)
    }

    pub fn eat(&mut self, word: &str) -> bool {
        let found = self.peek().is_some_and(|token| token.is_word(word));
        if found {
            self.next_token();
        }
        found
    }

    pub fn eat_sequence(&mut self, words: &[&str]) -> bool {
        let matches = words.len() <= self.tokens.len()
            && words
                .iter()
                .zip(self.tokens)
                .all(|(word, token)| token.is_word(word));
        if matches {
            self.tokens = self.tokens.get(words.len()..).unwrap_or_default();
        }
        matches
    }

    pub fn eat_any(&mut self, words: &[&str]) -> bool {
        words.iter().any(|word| self.eat(word))
    }

    pub fn qualified_name(&mut self) -> Vec<&'tokens Token> {
        let mut parts = Vec::new();
        while let Some(part) = self.peek().filter(|token| token.identifier().is_some()) {
            self.next_token();
            parts.push(part);
            if !self
                .peek()
                .is_some_and(|token| token.is_symbol(QUALIFIER_SEPARATOR))
            {
                break;
            }
            self.next_token();
        }
        parts
    }
}

pub fn normalized(parts: &[&Token]) -> Vec<String> {
    parts.iter().filter_map(|part| part.identifier()).collect()
}

pub fn last_raw(parts: &[&Token]) -> Option<String> {
    parts.last().map(|part| part.raw.clone())
}
