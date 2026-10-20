use super::{Script, parse};

const SINGLE_QUOTE: char = '\'';
const DOUBLE_QUOTE: char = '"';
const BACKSLASH: char = '\\';
const BACKTICK: char = '`';
const DOLLAR: char = '$';
const COMMENT: char = '#';
const OPERATOR_CHARACTERS: &[char] = &[';', '&', '|', '(', ')', '<', '>', '\n'];
const SEPARATORS: &[char] = &[';', '&', '(', ')', '\n'];
const PIPE: char = '|';
const REDIRECTS: &[char] = &['<', '>'];
const HEREDOC: &str = "<<";
const HERESTRING: &str = "<<<";
const EXPANSION: &str = "${VAR}";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Piece {
    Word(String),
    Nested(Script),
    Separator,
    Pipe,
}

struct Reader {
    characters: Vec<char>,
    index: usize,
    nested: Script,
    pending_heredocs: Vec<(String, bool)>,
}

pub fn pieces(script: &str) -> Vec<Piece> {
    let mut reader = Reader {
        characters: script.chars().collect(),
        index: 0,
        nested: Script::default(),
        pending_heredocs: Vec::new(),
    };
    let mut found = Vec::new();
    while let Some(character) = reader.peek() {
        if character == ' ' || character == '\t' || character == '\r' {
            reader.index = reader.index.saturating_add(1);
        } else if character == BACKSLASH && reader.peek_at(1) == Some('\n') {
            reader.index = reader.index.saturating_add(2);
        } else if character == COMMENT {
            reader.skip_comment();
        } else if OPERATOR_CHARACTERS.contains(&character) {
            found.extend(reader.operator());
        } else {
            let word = reader.word();
            if !word.is_empty() {
                found.push(Piece::Word(word));
            }
            if !reader.nested.commands.is_empty() || reader.nested.has_pipe {
                found.push(Piece::Nested(std::mem::take(&mut reader.nested)));
            }
        }
    }
    found
}

impl Reader {
    fn peek(&self) -> Option<char> {
        self.characters.get(self.index).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<char> {
        self.characters.get(self.index.saturating_add(offset)).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let character = self.peek()?;
        self.index = self.index.saturating_add(1);
        Some(character)
    }

    fn starts_with(&self, text: &str) -> bool {
        text.chars()
            .enumerate()
            .all(|(offset, expected)| self.peek_at(offset) == Some(expected))
    }

    fn skip_comment(&mut self) {
        self.skip_while(|character| character != '\n');
    }

    fn operator(&mut self) -> Option<Piece> {
        let character = self.bump()?;
        if character == '\n' {
            self.skip_pending_heredocs();
            return Some(Piece::Separator);
        }
        if REDIRECTS.contains(&character) {
            self.index = self.index.saturating_sub(1);
            self.redirect();
            return None;
        }
        if character == PIPE {
            if self.peek() == Some(PIPE) {
                self.bump();
                return Some(Piece::Separator);
            }
            if self.peek() == Some('&') {
                self.bump();
            }
            return Some(Piece::Pipe);
        }
        if character == '&' && self.peek() == Some('>') {
            self.redirect();
            return None;
        }
        while self
            .peek()
            .is_some_and(|next| next == character && SEPARATORS.contains(&next))
        {
            self.bump();
        }
        Some(Piece::Separator)
    }

    fn redirect(&mut self) {
        let heredoc = self.starts_with(HEREDOC) && !self.starts_with(HERESTRING);
        while self
            .peek()
            .is_some_and(|next| REDIRECTS.contains(&next) || next == '&' || next == '|')
        {
            self.bump();
        }
        let strips_tabs = heredoc && self.peek() == Some('-');
        if strips_tabs {
            self.bump();
        }
        while self.peek().is_some_and(|next| next == ' ' || next == '\t') {
            self.bump();
        }
        if self
            .peek()
            .is_some_and(|next| !OPERATOR_CHARACTERS.contains(&next))
        {
            let target = self.word();
            if heredoc {
                self.pending_heredocs.push((target, strips_tabs));
            }
        }
    }

    fn text(&self, start: usize, end: usize) -> String {
        self.characters
            .get(start..end)
            .unwrap_or_default()
            .iter()
            .collect()
    }

    fn skip_while(&mut self, keep: impl Fn(char) -> bool) {
        while self.peek().is_some_and(&keep) {
            self.bump();
        }
    }

    fn take_line(&mut self) -> (String, bool) {
        let start = self.index;
        self.skip_while(|character| character != '\n');
        let line = self.text(start, self.index);
        (line, self.bump().is_none())
    }

    fn skip_pending_heredocs(&mut self) {
        for (delimiter, strips_tabs) in std::mem::take(&mut self.pending_heredocs) {
            self.skip_heredoc(&delimiter, strips_tabs);
        }
    }

    fn skip_heredoc(&mut self, delimiter: &str, strips_tabs: bool) {
        loop {
            let (line, ended) = self.take_line();
            let candidate = if strips_tabs {
                line.trim_start_matches('\t')
            } else {
                line.as_str()
            };
            if candidate == delimiter || ended {
                return;
            }
        }
    }

    fn word(&mut self) -> String {
        let mut word = String::new();
        while let Some(character) = self.peek().filter(|character| !ends_word(*character)) {
            self.bump();
            match character {
                SINGLE_QUOTE => self.single_quoted(&mut word),
                DOUBLE_QUOTE => self.double_quoted(&mut word),
                BACKSLASH => self.escaped(&mut word),
                BACKTICK => self.backticked(&mut word),
                DOLLAR => self.dollar(&mut word),
                _ => word.push(character),
            }
        }
        let descriptor = !word.is_empty() && word.chars().all(|digit| digit.is_ascii_digit());
        if descriptor && self.peek().is_some_and(|next| REDIRECTS.contains(&next)) {
            word.clear();
        }
        word
    }

    fn escaped(&mut self, word: &mut String) {
        if let Some(escaped) = self.bump().filter(|escaped| *escaped != '\n') {
            word.push(escaped);
        }
    }

    fn single_quoted(&mut self, word: &mut String) {
        while let Some(character) = self.bump() {
            if character == SINGLE_QUOTE {
                return;
            }
            word.push(character);
        }
    }

    fn double_quoted(&mut self, word: &mut String) {
        while let Some(character) = self.bump() {
            match character {
                DOUBLE_QUOTE => return,
                BACKSLASH => self.escaped(word),
                BACKTICK => self.backticked(word),
                DOLLAR => self.dollar(word),
                _ => word.push(character),
            }
        }
    }

    fn backticked(&mut self, word: &mut String) {
        let start = self.index;
        self.skip_while(|character| character != BACKTICK);
        let inner = self.text(start, self.index);
        self.bump();
        self.absorb(parse(&inner));
        word.push_str(EXPANSION);
    }

    fn absorb(&mut self, script: Script) {
        self.nested.has_pipe |= script.has_pipe;
        self.nested.commands.extend(script.commands);
    }

    fn dollar(&mut self, word: &mut String) {
        match self.peek() {
            Some('(') => {
                let arithmetic = self.peek_at(1) == Some('(');
                let inner = self.balanced('(', ')');
                if arithmetic {
                    word.push_str(EXPANSION);
                } else {
                    let script = parse(&inner);
                    push_variable(word, &substitution_text(&script));
                    self.absorb(script);
                }
            }
            Some('{') => {
                let inner = self.balanced('{', '}');
                push_variable(word, &inner);
            }
            Some(next) if is_name_character(next) => {
                let start = self.index;
                self.skip_while(is_name_character);
                let name = self.text(start, self.index);
                push_variable(word, &name);
            }
            Some(_) | None => word.push(DOLLAR),
        }
    }

    fn balanced(&mut self, open: char, close: char) -> String {
        self.bump();
        let start = self.index;
        let mut depth = 1_usize;
        while let Some(character) = self.bump() {
            if character == SINGLE_QUOTE || character == DOUBLE_QUOTE {
                self.skip_while(|inner| inner != character);
                self.bump();
            } else if character == open {
                depth = depth.saturating_add(1);
            } else if character == close {
                depth = depth.saturating_sub(1);
            }
            if depth == 0 {
                return self.text(start, self.index.saturating_sub(1));
            }
        }
        self.text(start, self.characters.len())
    }
}

fn ends_word(character: char) -> bool {
    character == ' ' || character == '\t' || character == '\r' || OPERATOR_CHARACTERS.contains(&character)
}

fn is_name_character(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

fn substitution_text(script: &Script) -> String {
    match script.commands.as_slice() {
        [command] if !script.has_pipe => std::iter::once(&command.name)
            .chain(&command.arguments)
            .cloned()
            .collect(),
        _ => String::new(),
    }
}

fn push_variable(word: &mut String, name: &str) {
    word.push_str("${");
    word.push_str(name);
    word.push('}');
}
