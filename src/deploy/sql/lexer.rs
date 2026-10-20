use std::iter::Peekable;
use std::str::CharIndices;

const LINE_COMMENT: &str = "--";
const BLOCK_COMMENT_OPEN: &str = "/*";
const BLOCK_COMMENT_CLOSE: &str = "*/";
const STRING_QUOTE: char = '\'';
const IDENTIFIER_QUOTE: char = '"';
const DOLLAR: char = '$';
const ESCAPED_STRING_PREFIXES: &[char] = &['e', 'E'];
const STRING_PREFIXES: &[char] = &['e', 'E', 'b', 'B', 'x', 'X', 'n', 'N', 'u', 'U'];
const BACKSLASH: char = '\\';

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Word(String),
    Quoted(String),
    Literal,
    Symbol(char),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub raw: String,
    pub line: usize,
    pub column: usize,
}

impl Token {
    pub fn word(&self) -> Option<&str> {
        match &self.kind {
            TokenKind::Word(word) => Some(word),
            TokenKind::Quoted(_) | TokenKind::Literal | TokenKind::Symbol(_) => None,
        }
    }

    pub fn is_word(&self, expected: &str) -> bool {
        self.word() == Some(expected)
    }

    pub fn is_symbol(&self, expected: char) -> bool {
        self.kind == TokenKind::Symbol(expected)
    }

    pub fn identifier(&self) -> Option<String> {
        match &self.kind {
            TokenKind::Word(word) => Some(word.clone()),
            TokenKind::Quoted(name) => Some(name.clone()),
            TokenKind::Literal | TokenKind::Symbol(_) => None,
        }
    }
}

struct Lexer<'source> {
    source: &'source str,
    characters: Peekable<CharIndices<'source>>,
    line: usize,
    column: usize,
    tokens: Vec<Token>,
}

pub fn tokenize(source: &str, first_line: usize) -> Vec<Token> {
    let mut lexer = Lexer {
        source,
        characters: source.char_indices().peekable(),
        line: first_line,
        column: 1,
        tokens: Vec::new(),
    };
    while let Some(&(start, character)) = lexer.characters.peek() {
        lexer.next_token(start, character);
    }
    lexer.tokens
}

impl Lexer<'_> {
    fn advance(&mut self) -> Option<char> {
        let (_, character) = self.characters.next()?;
        if character == '\n' {
            self.line = self.line.saturating_add(1);
            self.column = 1;
        } else {
            self.column = self.column.saturating_add(1);
        }
        Some(character)
    }

    fn rest(&self, start: usize) -> &str {
        self.source.get(start..).unwrap_or_default()
    }

    fn position(&mut self) -> usize {
        self.characters
            .peek()
            .map_or(self.source.len(), |&(index, _)| index)
    }

    fn next_token(&mut self, start: usize, character: char) {
        let (line, column) = (self.line, self.column);
        let rest = self.rest(start);
        let kind = if character.is_whitespace() {
            self.advance();
            return;
        } else if rest.starts_with(LINE_COMMENT) {
            self.skip_line();
            return;
        } else if rest.starts_with(BLOCK_COMMENT_OPEN) {
            self.skip_block_comment();
            return;
        } else if character == STRING_QUOTE {
            self.skip_string(false);
            TokenKind::Literal
        } else if character == IDENTIFIER_QUOTE {
            TokenKind::Quoted(self.quoted_identifier())
        } else if character == DOLLAR {
            self.dollar(start)
        } else if character.is_ascii_digit() {
            self.skip_while(|next| next.is_ascii_alphanumeric() || next == '.');
            TokenKind::Literal
        } else if is_word_start(character) {
            self.word(start, character)
        } else {
            self.advance();
            TokenKind::Symbol(character)
        };
        let end = self.position();
        let raw = self.source.get(start..end).unwrap_or_default().to_owned();
        self.tokens.push(Token {
            kind,
            raw,
            line,
            column,
        });
    }

    fn skip_while(&mut self, keep: impl Fn(char) -> bool) {
        while self.characters.peek().is_some_and(|&(_, next)| keep(next)) {
            self.advance();
        }
    }

    fn skip_line(&mut self) {
        self.skip_while(|next| next != '\n');
    }

    fn eat_character(&mut self, expected: char) -> bool {
        let found = self.characters.peek().is_some_and(|&(_, next)| next == expected);
        if found {
            self.advance();
        }
        found
    }

    fn skip_block_comment(&mut self) {
        let mut depth = 0_usize;
        while let Some(&(index, _)) = self.characters.peek() {
            let rest = self.rest(index);
            let opens = rest.starts_with(BLOCK_COMMENT_OPEN);
            let closes = !opens && rest.starts_with(BLOCK_COMMENT_CLOSE);
            self.advance();
            if !(opens || closes) {
                continue;
            }
            self.advance();
            depth = if opens {
                depth.saturating_add(1)
            } else {
                depth.saturating_sub(1)
            };
            if depth == 0 {
                return;
            }
        }
    }

    fn skip_string(&mut self, backslash_escapes: bool) {
        self.advance();
        while let Some(character) = self.advance() {
            if backslash_escapes && character == BACKSLASH {
                self.advance();
                continue;
            }
            if character == STRING_QUOTE && !self.eat_character(STRING_QUOTE) {
                return;
            }
        }
    }

    fn quoted_identifier(&mut self) -> String {
        self.advance();
        let mut name = String::new();
        while let Some(character) = self.advance() {
            if character == IDENTIFIER_QUOTE && !self.eat_character(IDENTIFIER_QUOTE) {
                return name;
            }
            name.push(character);
        }
        name
    }

    fn dollar(&mut self, start: usize) -> TokenKind {
        let rest = self.rest(start);
        let Some(tag) = dollar_tag(rest) else {
            self.advance();
            self.skip_while(|next| next.is_ascii_digit());
            return TokenKind::Literal;
        };
        let tag = tag.to_owned();
        let body_start = start.saturating_add(tag.len());
        let closing = self
            .rest(body_start)
            .find(&tag)
            .map_or(self.source.len(), |offset| {
                body_start.saturating_add(offset).saturating_add(tag.len())
            });
        while self.position() < closing {
            self.advance();
        }
        TokenKind::Literal
    }

    fn word(&mut self, start: usize, character: char) -> TokenKind {
        self.advance();
        let next = self.characters.peek().map(|&(_, next)| next);
        if STRING_PREFIXES.contains(&character) && next == Some(STRING_QUOTE) {
            self.skip_string(ESCAPED_STRING_PREFIXES.contains(&character));
            return TokenKind::Literal;
        }
        self.skip_while(is_word_part);
        let end = self.position();
        TokenKind::Word(self.source.get(start..end).unwrap_or_default().to_lowercase())
    }
}

fn dollar_tag(text: &str) -> Option<&str> {
    let after = text.strip_prefix(DOLLAR)?;
    let name_length = after
        .char_indices()
        .find(|&(_, character)| !(character.is_alphanumeric() || character == '_'))
        .map_or(after.len(), |(index, _)| index);
    let name = after.get(..name_length)?;
    if name.starts_with(|first: char| first.is_ascii_digit()) {
        return None;
    }
    after
        .get(name_length..)?
        .starts_with(DOLLAR)
        .then(|| text.get(..name_length.saturating_add(2)))
        .flatten()
}

fn is_word_start(character: char) -> bool {
    character.is_alphabetic() || character == '_'
}

fn is_word_part(character: char) -> bool {
    character.is_alphanumeric() || character == '_' || character == DOLLAR
}

#[cfg(test)]
mod tests {
    use super::{TokenKind, tokenize};

    fn kinds(source: &str) -> Vec<TokenKind> {
        tokenize(source, 1).into_iter().map(|token| token.kind).collect()
    }

    #[test]
    fn keeps_strings_comments_and_dollar_bodies_out_of_the_statement() {
        let tokens = kinds("drop -- drop table\n/* drop /* nested */ */ 'it''s' $$ drop; $$ E'a\\'b';");
        assert_eq!(
            tokens,
            vec![
                TokenKind::Word("drop".to_owned()),
                TokenKind::Literal,
                TokenKind::Literal,
                TokenKind::Literal,
                TokenKind::Symbol(';'),
            ]
        );
    }

    #[test]
    fn lowercases_words_and_keeps_quoted_names() {
        let tokens = tokenize("ALTER \"MyTable\" $1 $body$x$body$", 3);
        assert_eq!(tokens[0].kind, TokenKind::Word("alter".to_owned()));
        assert_eq!(tokens[1].kind, TokenKind::Quoted("MyTable".to_owned()));
        assert_eq!(tokens[1].raw, "\"MyTable\"");
        assert_eq!(tokens[2].kind, TokenKind::Literal);
        assert_eq!(tokens[3].kind, TokenKind::Literal);
        assert_eq!((tokens[3].line, tokens[3].column), (3, 20));
    }

    #[test]
    fn counts_lines_and_columns() {
        let tokens = tokenize("a\n  b", 1);
        assert_eq!((tokens[1].line, tokens[1].column), (2, 3));
    }
}
