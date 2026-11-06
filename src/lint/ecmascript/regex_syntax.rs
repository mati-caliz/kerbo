mod classes;
mod escapes;
mod tree;

pub use tree::{Kind, Mode, Reference, RegexNode, RegexTree};
use tree::{check_duplicate_names, check_references};

const GROUP_NAME_EXTRAS: &[char] = &['$', '_', '\u{200c}', '\u{200d}'];

struct Parser {
    characters: Vec<(usize, char)>,
    length: usize,
    position: usize,
    mode: Mode,
    capture_count: usize,
    has_named_groups: bool,
    tree: RegexTree,
}

// `RegExpParser.parsePattern`: el error dice qué regla de la gramática no se cumple.
pub fn parse(pattern: &str, mode: Mode) -> Result<RegexTree, &'static str> {
    let characters: Vec<(usize, char)> = pattern.char_indices().collect();
    let (capture_count, has_named_groups) = count_captures(&characters);
    let mut parser = Parser {
        characters,
        length: pattern.len(),
        position: 0,
        mode,
        capture_count,
        has_named_groups,
        tree: RegexTree::default(),
    };
    let root = parser.open(Kind::Pattern, None);
    parser.disjunction(root)?;
    if parser.peek().is_some() {
        return Err("Unmatched ')'");
    }
    parser.close(root);
    check_references(&parser.tree)?;
    check_duplicate_names(&parser.tree)?;
    Ok(parser.tree)
}

fn count_captures(characters: &[(usize, char)]) -> (usize, bool) {
    let mut count = 0_usize;
    let mut named = false;
    let mut in_class = false;
    let mut index = 0;
    while let Some((_, character)) = characters.get(index) {
        match character {
            '\\' => index = index.saturating_add(1),
            '[' => in_class = true,
            ']' => in_class = false,
            '(' if !in_class => {
                let next = characters.get(index.saturating_add(1)).map(|(_, next)| *next);
                let after = characters.get(index.saturating_add(2)).map(|(_, after)| *after);
                let third = characters.get(index.saturating_add(3)).map(|(_, third)| *third);
                if next != Some('?') {
                    count = count.saturating_add(1);
                } else if after == Some('<') && !matches!(third, Some('=' | '!')) {
                    count = count.saturating_add(1);
                    named = true;
                }
            }
            _ => {}
        }
        index = index.saturating_add(1);
    }
    (count, named)
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.characters
            .get(self.position)
            .map(|(_, character)| *character)
    }

    fn peek_at(&self, offset: usize) -> Option<char> {
        self.characters
            .get(self.position.saturating_add(offset))
            .map(|(_, character)| *character)
    }

    fn byte_position(&self) -> usize {
        self.characters
            .get(self.position)
            .map_or(self.length, |(byte, _)| *byte)
    }

    fn advance(&mut self) -> Option<char> {
        let character = self.peek()?;
        self.position = self.position.saturating_add(1);
        Some(character)
    }

    fn eat(&mut self, expected: char) -> bool {
        let matches = self.peek() == Some(expected);
        if matches {
            self.position = self.position.saturating_add(1);
        }
        matches
    }

    fn eat_text(&mut self, expected: &str) -> bool {
        let matches = expected
            .chars()
            .enumerate()
            .all(|(offset, character)| self.peek_at(offset) == Some(character));
        if matches {
            self.position = self.position.saturating_add(expected.chars().count());
        }
        matches
    }

    fn open(&mut self, kind: Kind, parent: Option<usize>) -> usize {
        let start = self.byte_position();
        self.tree.nodes.push(RegexNode {
            kind,
            parent,
            start,
            end: start,
        });
        self.tree.nodes.len().saturating_sub(1)
    }

    fn close(&mut self, node: usize) {
        let end = self.byte_position();
        if let Some(found) = self.tree.nodes.get_mut(node) {
            found.end = end;
        }
    }

    fn disjunction(&mut self, parent: usize) -> Result<(), &'static str> {
        loop {
            let alternative = self.open(Kind::Alternative, Some(parent));
            self.alternative(alternative)?;
            self.close(alternative);
            if !self.eat('|') {
                return Ok(());
            }
        }
    }

    fn alternative(&mut self, parent: usize) -> Result<(), &'static str> {
        while let Some(character) = self.peek() {
            if matches!(character, ')' | '|') {
                return Ok(());
            }
            let quantifiable = self.term(parent)?;
            let found_quantifier = self.quantifier()?;
            if found_quantifier && !quantifiable {
                return Err("Nothing to repeat");
            }
        }
        Ok(())
    }

    // Devuelve si lo que acaba de leer admite un cuantificador.
    fn term(&mut self, parent: usize) -> Result<bool, &'static str> {
        let strict = self.mode.is_strict();
        match self.peek() {
            Some('^' | '$') => {
                self.advance();
                Ok(false)
            }
            Some('(') => self.group(parent),
            Some('[') => {
                classes::class(self, parent)?;
                Ok(true)
            }
            Some('\\') => escapes::escape(self, parent),
            Some('*' | '+' | '?') => Err("Nothing to repeat"),
            Some('{') if strict => Err("Nothing to repeat"),
            Some('{') if self.looks_like_braced_quantifier() => Err("Nothing to repeat"),
            Some(']' | '}') if strict => Err("Lone quantifier brackets"),
            _ => {
                self.advance();
                Ok(true)
            }
        }
    }

    fn looks_like_braced_quantifier(&self) -> bool {
        let mut offset = 1;
        let digits = |parser: &Self, mut at: usize| {
            let first = at;
            while parser.peek_at(at).is_some_and(|digit| digit.is_ascii_digit()) {
                at = at.saturating_add(1);
            }
            (at, at > first)
        };
        let (after, has_digits) = digits(self, offset);
        offset = after;
        if !has_digits {
            return false;
        }
        if self.peek_at(offset) == Some(',') {
            offset = digits(self, offset.saturating_add(1)).0;
        }
        self.peek_at(offset) == Some('}')
    }

    fn quantifier(&mut self) -> Result<bool, &'static str> {
        let found = match self.peek() {
            Some('*' | '+' | '?') => {
                self.advance();
                true
            }
            Some('{') => self.braced_quantifier()?,
            _ => false,
        };
        if found {
            self.eat('?');
        }
        Ok(found)
    }

    fn number(&mut self) -> Option<u64> {
        let mut digits = String::new();
        while let Some(digit) = self.peek().filter(char::is_ascii_digit) {
            digits.push(digit);
            self.advance();
        }
        digits
            .parse()
            .ok()
            .or_else(|| (!digits.is_empty()).then_some(u64::MAX))
    }

    fn braced_quantifier(&mut self) -> Result<bool, &'static str> {
        if !self.looks_like_braced_quantifier() {
            return if self.mode.is_strict() {
                Err("Incomplete quantifier")
            } else {
                self.advance();
                Ok(false)
            };
        }
        self.advance();
        let minimum = self.number();
        let maximum = if self.eat(',') { self.number() } else { minimum };
        self.eat('}');
        if let (Some(minimum), Some(maximum)) = (minimum, maximum)
            && minimum > maximum
        {
            return Err("numbers out of order in {} quantifier");
        }
        Ok(true)
    }

    fn group(&mut self, parent: usize) -> Result<bool, &'static str> {
        let start_position = self.position;
        let node = self.open(Kind::Group, Some(parent));
        self.advance();
        let mut quantifiable = true;
        if self.eat('?') {
            if self.eat_text("<=") || self.eat_text("<!") {
                let negate = self.peek_at(0).is_some() && self.previous_is('!');
                self.set_kind(node, Kind::Look { behind: true, negate });
                quantifiable = false;
            } else if self.eat('=') || self.eat('!') {
                let negate = self.previous_is('!');
                self.set_kind(
                    node,
                    Kind::Look {
                        behind: false,
                        negate,
                    },
                );
                quantifiable = !self.mode.is_strict();
            } else if self.eat('<') {
                let name = self.group_name()?;
                self.set_kind(node, Kind::Capture(Some(name)));
            } else if !self.eat(':') && !self.modifiers() {
                return Err("Invalid group");
            }
        } else {
            self.set_kind(node, Kind::Capture(None));
        }
        self.disjunction(node)?;
        if !self.eat(')') {
            self.position = start_position;
            return Err("Unterminated group");
        }
        self.close(node);
        Ok(quantifiable)
    }

    fn previous_is(&self, expected: char) -> bool {
        self.position
            .checked_sub(1)
            .and_then(|previous| self.characters.get(previous))
            .is_some_and(|(_, character)| *character == expected)
    }

    fn set_kind(&mut self, node: usize, kind: Kind) {
        if let Some(found) = self.tree.nodes.get_mut(node) {
            found.kind = kind;
        }
    }

    // `(?i:…)` y `(?-i:…)`.
    fn modifiers(&mut self) -> bool {
        let mut offset = 0;
        while self.peek_at(offset).is_some_and(|flag| "ims-".contains(flag)) {
            offset = offset.saturating_add(1);
        }
        if offset == 0 || self.peek_at(offset) != Some(':') {
            return false;
        }
        self.position = self.position.saturating_add(offset).saturating_add(1);
        true
    }

    fn group_name(&mut self) -> Result<String, &'static str> {
        let mut name = String::new();
        while let Some(character) = self.advance().filter(|character| *character != '>') {
            name.push(character);
        }
        if self.previous_is('>') && is_group_name(&name) {
            Ok(name)
        } else {
            Err("Invalid capture group name")
        }
    }
}

fn is_group_name(name: &str) -> bool {
    let mut letters = name.chars();
    let valid_start = letters
        .next()
        .is_some_and(|first| first.is_alphabetic() || matches!(first, '$' | '_'));
    valid_start && letters.all(|letter| letter.is_alphanumeric() || GROUP_NAME_EXTRAS.contains(&letter))
}
