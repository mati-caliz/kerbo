use std::iter::Peekable;
use std::str::Chars;

const STATEMENT_SEPARATOR: char = ';';
const LINE_COMMENT: char = '-';
const BLOCK_COMMENT_START: char = '/';
const BLOCK_COMMENT_STAR: char = '*';
const STRING_QUOTE: char = '\'';
const IDENTIFIER_QUOTE: char = '"';
const DOLLAR: char = '$';
const QUALIFIER: char = '.';
const OPEN: char = '(';
const CLOSE: char = ')';
const SEPARATOR: char = ',';

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    Word(String),
    Quoted(String),
    Symbol(char),
    Literal,
}

impl Token {
    pub fn is(&self, keyword: &str) -> bool {
        matches!(self, Self::Word(word) if word == keyword)
    }

    fn identifier(&self) -> Option<&str> {
        match self {
            Self::Word(text) | Self::Quoted(text) => Some(text),
            Self::Symbol(_) | Self::Literal => None,
        }
    }
}

fn is_word_character(character: char) -> bool {
    character.is_alphanumeric() || character == '_' || character == DOLLAR
}

fn starts_dollar_tag(character: char) -> bool {
    character == DOLLAR || character.is_alphabetic() || character == '_'
}

fn skip_line(characters: &mut Peekable<Chars<'_>>) {
    for character in characters.by_ref() {
        if character == '\n' {
            break;
        }
    }
}

fn skip_block(characters: &mut Peekable<Chars<'_>>) {
    let mut previous = None;
    for character in characters.by_ref() {
        if previous == Some(BLOCK_COMMENT_STAR) && character == BLOCK_COMMENT_START {
            break;
        }
        previous = Some(character);
    }
}

fn quoted(characters: &mut Peekable<Chars<'_>>, quote: char) -> String {
    let mut text = String::new();
    while let Some(character) = characters.next() {
        if character == quote {
            if characters.peek() == Some(&quote) {
                characters.next();
            } else {
                break;
            }
        }
        text.push(character);
    }
    text
}

fn dollar_quoted(characters: &mut Peekable<Chars<'_>>) {
    let mut tag = String::from(DOLLAR);
    for character in characters.by_ref() {
        tag.push(character);
        if character == DOLLAR {
            break;
        }
    }
    let mut body = String::new();
    for character in characters.by_ref() {
        body.push(character);
        if body.ends_with(&tag) {
            break;
        }
    }
}

fn word(characters: &mut Peekable<Chars<'_>>, first: char) -> String {
    let mut text = first.to_lowercase().collect::<String>();
    while let Some(character) = characters.next_if(|next| is_word_character(*next)) {
        text.extend(character.to_lowercase());
    }
    text
}

pub fn statements(sql: &str) -> Vec<Vec<Token>> {
    let mut statements = Vec::new();
    let mut current = Vec::new();
    let mut characters = sql.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            STATEMENT_SEPARATOR => statements.push(std::mem::take(&mut current)),
            LINE_COMMENT if characters.peek() == Some(&LINE_COMMENT) => skip_line(&mut characters),
            BLOCK_COMMENT_START if characters.peek() == Some(&BLOCK_COMMENT_STAR) => {
                characters.next();
                skip_block(&mut characters);
            }
            STRING_QUOTE => {
                quoted(&mut characters, STRING_QUOTE);
                current.push(Token::Literal);
            }
            IDENTIFIER_QUOTE => current.push(Token::Quoted(quoted(&mut characters, IDENTIFIER_QUOTE))),
            DOLLAR if characters.peek().is_some_and(|next| starts_dollar_tag(*next)) => {
                dollar_quoted(&mut characters);
                current.push(Token::Literal);
            }
            _ if character.is_whitespace() => {}
            _ if is_word_character(character) => current.push(Token::Word(word(&mut characters, character))),
            _ => current.push(Token::Symbol(character)),
        }
    }
    statements.push(current);
    statements.retain(|statement| !statement.is_empty());
    statements
}

pub struct Cursor<'tokens> {
    tokens: &'tokens [Token],
}

impl<'tokens> Cursor<'tokens> {
    pub const fn new(tokens: &'tokens [Token]) -> Self {
        Self { tokens }
    }

    fn advance(&mut self, count: usize) {
        self.tokens = self.tokens.get(count..).unwrap_or_default();
    }

    pub fn keyword(&mut self, keyword: &str) -> bool {
        let matched = self.tokens.first().is_some_and(|token| token.is(keyword));
        if matched {
            self.advance(1);
        }
        matched
    }

    pub fn keywords(&mut self, keywords: &[&str]) -> bool {
        let matched = keywords.len() <= self.tokens.len()
            && self
                .tokens
                .iter()
                .zip(keywords)
                .all(|(token, keyword)| token.is(keyword));
        if matched {
            self.advance(keywords.len());
        }
        matched
    }

    pub fn identifier(&mut self) -> Option<String> {
        let mut name = self.tokens.first()?.identifier()?.to_owned();
        self.advance(1);
        while let [Token::Symbol(QUALIFIER), next, ..] = self.tokens {
            let Some(part) = next.identifier() else {
                break;
            };
            part.clone_into(&mut name);
            self.advance(2);
        }
        Some(name)
    }

    pub fn identifiers(&mut self) -> Vec<String> {
        let mut found = Vec::new();
        while let Some(identifier) = self.identifier() {
            found.push(identifier);
            if !self.symbol(SEPARATOR) {
                break;
            }
        }
        found
    }

    pub fn symbol(&mut self, symbol: char) -> bool {
        let matched = self.tokens.first() == Some(&Token::Symbol(symbol));
        if matched {
            self.advance(1);
        }
        matched
    }

    pub fn at_symbol(&self, symbol: char) -> bool {
        self.tokens.first() == Some(&Token::Symbol(symbol))
    }

    pub fn at_keyword(&self, keyword: &str) -> bool {
        self.tokens.first().is_some_and(|token| token.is(keyword))
    }

    pub fn at_any_keyword(&self, keywords: &[&str]) -> bool {
        keywords.iter().any(|keyword| self.at_keyword(keyword))
    }

    pub fn contains(&self, keyword: &str) -> bool {
        self.tokens.iter().any(|token| token.is(keyword))
    }

    pub fn contains_sequence(&self, keywords: &[&str]) -> bool {
        self.tokens.windows(keywords.len()).any(|window| {
            window
                .iter()
                .zip(keywords)
                .all(|(token, keyword)| token.is(keyword))
        })
    }

    pub fn skip_until(&mut self, symbol: char) {
        let position = self
            .tokens
            .iter()
            .position(|token| *token == Token::Symbol(symbol))
            .unwrap_or(self.tokens.len());
        self.advance(position);
    }

    pub fn skip_until_keyword(&mut self, keyword: &str) {
        let position = self
            .tokens
            .iter()
            .position(|token| token.is(keyword))
            .unwrap_or(self.tokens.len());
        self.advance(position);
    }

    pub fn elements(&mut self) -> Vec<&'tokens [Token]> {
        self.skip_until(OPEN);
        if !self.symbol(OPEN) {
            return Vec::new();
        }
        let tokens = self.tokens;
        let mut elements = Vec::new();
        let mut depth = 0_usize;
        let mut start = 0;
        for (position, token) in tokens.iter().enumerate() {
            match token {
                Token::Symbol(OPEN) => depth += 1,
                Token::Symbol(CLOSE) if depth == 0 => {
                    elements.extend(tokens.get(start..position));
                    self.advance(position + 1);
                    return elements;
                }
                Token::Symbol(CLOSE) => depth -= 1,
                Token::Symbol(SEPARATOR) if depth == 0 => {
                    elements.extend(tokens.get(start..position));
                    start = position + 1;
                }
                Token::Word(_) | Token::Quoted(_) | Token::Symbol(_) | Token::Literal => {}
            }
        }
        elements.extend(tokens.get(start..));
        self.advance(tokens.len());
        elements
    }

    pub fn actions(&self) -> Vec<&'tokens [Token]> {
        let mut actions = Vec::new();
        let mut depth = 0_usize;
        let mut start = 0;
        for (position, token) in self.tokens.iter().enumerate() {
            match token {
                Token::Symbol(OPEN) => depth += 1,
                Token::Symbol(CLOSE) => depth = depth.saturating_sub(1),
                Token::Symbol(SEPARATOR) if depth == 0 => {
                    actions.extend(self.tokens.get(start..position));
                    start = position + 1;
                }
                Token::Word(_) | Token::Quoted(_) | Token::Symbol(_) | Token::Literal => {}
            }
        }
        actions.extend(self.tokens.get(start..));
        actions
    }

    pub fn single_identifier(tokens: &[Token]) -> Option<String> {
        let mut cursor = Cursor::new(tokens);
        let identifier = cursor.identifier()?;
        cursor.tokens.is_empty().then_some(identifier)
    }

    pub fn render(tokens: &[Token]) -> String {
        tokens
            .iter()
            .map(|token| match token {
                Token::Word(text) | Token::Quoted(text) => text.clone(),
                Token::Symbol(symbol) => symbol.to_string(),
                Token::Literal => String::from(STRING_QUOTE),
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}
