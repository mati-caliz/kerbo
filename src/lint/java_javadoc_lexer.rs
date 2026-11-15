use super::java_javadoc_parse::ParseError;

const COMMENT_START: &str = "<!--";
const COMMENT_END: &str = "-->";
const INLINE_START: &str = "{@";
const UNRECOGNIZED: &str = "token recognition error at";

// El lexer de `JavadocCommentsLexer.g4` en lo que toca al HTML: texto, etiquetas en línea y
// comentarios enteros, y el modo TAG con sus errores de reconocimiento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Text,
    Inline,
    Comment,
    Open,
    Slash,
    Name,
    AttributeName,
    Equals,
    Value,
    Close,
    SlashClose,
    Blank,
}

#[derive(Debug, Clone)]
pub(super) struct Token {
    pub(super) kind: Kind,
    pub(super) row: usize,
    pub(super) text: String,
}

pub(super) struct Lexer {
    chars: Vec<char>,
    position: usize,
    row: usize,
    after_newline: bool,
    pub(super) tokens: Vec<Token>,
    pub(super) last_error: Option<ParseError>,
}

impl Lexer {
    fn peek(&self, offset: usize) -> Option<char> {
        self.chars.get(self.position + offset).copied()
    }

    fn starts_with(&self, wanted: &str) -> bool {
        wanted
            .chars()
            .enumerate()
            .all(|(offset, letter)| self.peek(offset) == Some(letter))
    }

    fn emit(&mut self, kind: Kind, text: String) {
        self.tokens.push(Token {
            kind,
            row: self.row,
            text,
        });
        self.after_newline = false;
    }

    fn newline(&mut self) -> bool {
        match self.peek(0) {
            Some('\r') => {
                self.position += if self.peek(1) == Some('\n') { 2 } else { 1 };
            }
            Some('\n') => self.position += 1,
            _ => return false,
        }
        self.row += 1;
        self.after_newline = true;
        true
    }

    fn leading_asterisks(&mut self) -> bool {
        if !self.after_newline {
            return false;
        }
        let mut offset = 0;
        while matches!(self.peek(offset), Some(' ' | '\t')) {
            offset += 1;
        }
        if self.peek(offset) != Some('*') {
            return false;
        }
        while self.peek(offset) == Some('*') {
            offset += 1;
        }
        self.position += offset;
        self.after_newline = false;
        true
    }

    pub(super) fn new(body: &str, first_row: usize) -> Self {
        Self {
            chars: body.chars().collect(),
            position: 0,
            row: first_row,
            after_newline: false,
            tokens: Vec::new(),
            last_error: None,
        }
    }

    pub(super) fn run(&mut self) {
        let mut text = String::new();
        while self.position < self.chars.len() {
            if self.newline() {
                self.flush_text(&mut text);
                continue;
            }
            if text.is_empty() && self.leading_asterisks() {
                continue;
            }
            let starts_markup = self.starts_with(COMMENT_START)
                || self.starts_with(INLINE_START)
                || (self.peek(0) == Some('<')
                    && self
                        .peek(1)
                        .is_some_and(|next| next.is_alphabetic() || next == '/' || next == '!'));
            if !starts_markup {
                text.extend(self.peek(0));
                self.position += 1;
                continue;
            }
            self.flush_text(&mut text);
            if self.starts_with(COMMENT_START) {
                self.comment();
            } else if self.starts_with(INLINE_START) {
                self.inline_tag();
            } else {
                self.emit(Kind::Open, "<".to_owned());
                self.position += 1;
                self.tag();
            }
        }
        self.flush_text(&mut text);
    }

    fn flush_text(&mut self, text: &mut String) {
        if text.is_empty() {
            return;
        }
        let kind = if text.trim().is_empty() {
            Kind::Blank
        } else {
            Kind::Text
        };
        self.emit(kind, std::mem::take(text));
    }

    fn comment(&mut self) {
        let row = self.row;
        self.position += COMMENT_START.chars().count();
        while self.position < self.chars.len() && !self.starts_with(COMMENT_END) {
            if !self.newline() {
                self.position += 1;
            }
        }
        self.position += COMMENT_END.chars().count();
        self.tokens.push(Token {
            kind: Kind::Comment,
            row,
            text: String::new(),
        });
        self.after_newline = false;
    }

    fn inline_tag(&mut self) {
        let row = self.row;
        self.position += INLINE_START.chars().count();
        let mut depth = 1_usize;
        while self.position < self.chars.len() && depth > 0 {
            if self.newline() {
                continue;
            }
            match self.peek(0) {
                Some('{') => depth += 1,
                Some('}') => depth -= 1,
                _ => {}
            }
            self.position += 1;
        }
        self.tokens.push(Token {
            kind: Kind::Inline,
            row,
            text: String::new(),
        });
        self.after_newline = false;
    }

    // El modo TAG del lexer: nombres, `=`, `/`, `>`, `/>`, blancos y saltos; cualquier otro carácter
    // es un error de reconocimiento que se saltea.
    fn tag(&mut self) {
        let mut seen_name = false;
        while self.position < self.chars.len() {
            if self.newline() || self.leading_asterisks() {
                continue;
            }
            let Some(current) = self.peek(0) else {
                return;
            };
            if self.starts_with("/>") {
                self.emit(Kind::SlashClose, "/>".to_owned());
                self.position += 2;
                return;
            }
            match current {
                '>' => {
                    self.emit(Kind::Close, ">".to_owned());
                    self.position += 1;
                    return;
                }
                '/' => {
                    self.emit(Kind::Slash, "/".to_owned());
                    self.position += 1;
                }
                '=' => {
                    self.emit(Kind::Equals, "=".to_owned());
                    self.position += 1;
                    self.attribute_value();
                }
                ' ' | '\t' => {
                    self.emit(Kind::Blank, String::new());
                    self.position += 1;
                }
                letter if is_name_start(letter) => {
                    let name = self.read_name();
                    self.emit(name_kind(seen_name), name);
                    seen_name = true;
                }
                other => {
                    self.last_error = Some(ParseError {
                        row: self.row,
                        detail: format!("{UNRECOGNIZED}: '{other}'"),
                    });
                    self.position += 1;
                }
            }
        }
    }

    fn read_name(&mut self) -> String {
        let mut name = String::new();
        while let Some(next) = self.peek(0).filter(|next| is_name_char(*next)) {
            name.push(next);
            self.position += 1;
        }
        name
    }

    fn attribute_value(&mut self) {
        loop {
            if self.newline() || self.leading_asterisks() {
                continue;
            }
            let mut offset = 0;
            while self.peek(offset) == Some(' ') {
                offset += 1;
            }
            let length = self.quoted_length(offset).max(self.unquoted_length(offset));
            if length > 0 {
                self.position += offset + length;
                self.emit(Kind::Value, String::new());
                return;
            }
            let Some(other) = self.peek(offset) else {
                return;
            };
            self.last_error = Some(ParseError {
                row: self.row,
                detail: format!("{UNRECOGNIZED}: '{other}'"),
            });
            self.position += offset + 1;
        }
    }

    fn quoted_length(&self, start: usize) -> usize {
        let Some(quote) = self.peek(start).filter(|quote| *quote == '"' || *quote == '\'') else {
            return 0;
        };
        let mut offset = start + 1;
        while let Some(current) = self.peek(offset) {
            if current == quote {
                return offset + 1 - start;
            }
            if current == '<' && self.peek(offset + 1) == Some('/') {
                return 0;
            }
            offset += 1;
        }
        0
    }

    fn unquoted_length(&self, start: usize) -> usize {
        let mut offset = start;
        while let Some(current) = self.peek(offset) {
            let ends = matches!(current, '>' | ' ' | '}' | '\t' | '\r' | '\n')
                || (current == '/' && self.peek(offset + 1) == Some('>'));
            if ends {
                break;
            }
            offset += 1;
        }
        offset - start
    }
}

// El primer nombre de una etiqueta es el de la etiqueta; los que siguen, de atributos.
const fn name_kind(seen_name: bool) -> Kind {
    if seen_name {
        Kind::AttributeName
    } else {
        Kind::Name
    }
}

fn is_name_start(letter: char) -> bool {
    letter == ':' || letter.is_alphabetic()
}

fn is_name_char(letter: char) -> bool {
    is_name_start(letter) || matches!(letter, '-' | '_' | '.') || letter.is_ascii_digit()
}
