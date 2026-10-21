use tree_sitter::Node;

use crate::lint::ecmascript::nodes::text;

const GENERAL_ESCAPES: &str = "\\bcdDfnpPrsStvwWxu0123456789]";
const OUTSIDE_CLASS_ESCAPES: &str = "^/.$*+?[{}|()Bk";
const SYNTAX_CHARACTERS: &str = "^$\\.*+?()[]{}|/";
const CLASS_ESCAPES: &str = "dDsSwW";
const CONTROL_ESCAPES: &str = "fnrtv";
const UNICODE_FLAG: char = 'u';
const SETS_FLAG: char = 'v';
const NAMED_GROUP: &str = "(?<";
const HEX_DIGITS_BYTE: usize = 2;
const HEX_DIGITS_UNICODE: usize = 4;
const LOOKBEHIND_MARKS: &[char] = &['=', '!'];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Last {
    Nothing,
    Atom,
    Quantifier,
    LazyQuantifier,
}

#[derive(Debug, Clone, Copy)]
struct Pending {
    offset: usize,
    escaped: char,
    index: usize,
}

struct Scanner {
    characters: Vec<(usize, char)>,
    unicode: bool,
    named_groups: bool,
    position: usize,
    last: Last,
    found: Vec<(usize, char)>,
}

pub fn useless_escapes(node: Node<'_>, source: &[u8]) -> Vec<(usize, String)> {
    let (Some(pattern), flags) = (
        node.child_by_field_name("pattern"),
        node.child_by_field_name("flags")
            .map_or("", |flags| text(flags, source)),
    ) else {
        return Vec::new();
    };
    if flags.contains(SETS_FLAG) {
        return Vec::new();
    }
    let raw = text(pattern, source);
    let base = pattern.start_byte().saturating_sub(node.start_byte());
    let mut scanner = Scanner {
        characters: raw.char_indices().collect(),
        unicode: flags.contains(UNICODE_FLAG),
        named_groups: has_named_groups(raw),
        position: 0,
        last: Last::Nothing,
        found: Vec::new(),
    };
    if scanner.scan().is_none() {
        return Vec::new();
    }
    scanner
        .found
        .into_iter()
        .map(|(offset, escaped)| (base.saturating_add(offset), escaped.to_string()))
        .collect()
}

fn has_named_groups(raw: &str) -> bool {
    raw.match_indices(NAMED_GROUP).any(|(index, _)| {
        raw.get(index.saturating_add(NAMED_GROUP.len())..)
            .and_then(|rest| rest.chars().next())
            .is_some_and(|next| !LOOKBEHIND_MARKS.contains(&next))
    })
}

impl Scanner {
    fn peek(&self, ahead: usize) -> Option<char> {
        self.characters
            .get(self.position.saturating_add(ahead))
            .map(|(_, character)| *character)
    }

    fn offset(&self) -> usize {
        self.characters
            .get(self.position)
            .map_or(0, |(offset, _)| *offset)
    }

    const fn advance(&mut self, count: usize) {
        self.position = self.position.saturating_add(count);
    }

    fn scan(&mut self) -> Option<()> {
        while let Some(character) = self.peek(0) {
            match character {
                '\\' => {
                    self.escape(false, &mut Vec::new())?;
                    self.last = Last::Atom;
                }
                '[' => {
                    self.class()?;
                    self.last = Last::Atom;
                }
                '(' => self.group()?,
                '|' | '^' | '$' => {
                    self.advance(1);
                    self.last = Last::Nothing;
                }
                '*' | '+' | '?' => self.quantifier(character)?,
                '{' => self.braces()?,
                _ => {
                    self.advance(1);
                    self.last = Last::Atom;
                }
            }
        }
        Some(())
    }

    const fn quantifier(&mut self, character: char) -> Option<()> {
        self.advance(1);
        self.last = match (self.last, character) {
            (Last::Atom, _) => Last::Quantifier,
            (Last::Quantifier, '?') => Last::LazyQuantifier,
            _ => return None,
        };
        Some(())
    }

    fn braces(&mut self) -> Option<()> {
        let start = self.position;
        let rest: String = self
            .characters
            .iter()
            .skip(start)
            .map(|(_, character)| *character)
            .collect();
        let closing = rest.find('}');
        let bounds = closing.and_then(|end| rest.get(1..end)).filter(|inner| {
            !inner.is_empty()
                && inner
                    .chars()
                    .all(|character| character.is_ascii_digit() || character == ',')
        });
        match bounds {
            Some(inner) => {
                let mut parts = inner.splitn(2, ',');
                let minimum: Option<u64> = parts.next().and_then(|part| part.parse().ok());
                let maximum: Option<u64> = parts.next().and_then(|part| part.parse().ok());
                if minimum.is_none() || minimum.zip(maximum).is_some_and(|(low, high)| low > high) {
                    return None;
                }
                if self.last != Last::Atom {
                    return None;
                }
                self.advance(inner.chars().count().saturating_add(2));
                self.last = Last::Quantifier;
            }
            None if self.unicode => return None,
            None => {
                self.advance(1);
                self.last = Last::Atom;
            }
        }
        Some(())
    }

    fn group(&mut self) -> Option<()> {
        self.advance(1);
        if self.peek(0) == Some('?') {
            match (self.peek(1), self.peek(2)) {
                (Some('<'), Some('=' | '!')) => self.advance(3),
                (Some(':' | '=' | '!' | '<'), _) => self.advance(2),
                _ => return None,
            }
        }
        self.last = Last::Nothing;
        Some(())
    }

    fn class(&mut self) -> Option<()> {
        let start = self.position;
        self.advance(1);
        if self.peek(0) == Some('^') {
            self.advance(1);
        }
        let mut pending: Vec<Pending> = Vec::new();
        let mut previous: Option<char> = None;
        loop {
            let character = self.peek(0)?;
            match character {
                ']' => break,
                '\\' => {
                    self.escape(true, &mut pending)?;
                    previous = None;
                }
                '-' if previous.is_some() && self.peek(1).is_some_and(|next| next != ']' && next != '\\') => {
                    let ordered = previous.zip(self.peek(1)).is_some_and(|(low, high)| low <= high);
                    ordered.then_some(())?;
                    self.advance(2);
                    previous = None;
                }
                _ => {
                    previous = Some(character);
                    self.advance(1);
                }
            }
        }
        let closing = self.position;
        self.advance(1);
        for item in pending {
            let at_start = item.index == start.saturating_add(1);
            let allowed = match item.escaped {
                '^' => at_start,
                _ => !at_start && item.index.saturating_add(2) != closing,
            };
            if !allowed {
                self.found.push((item.offset, item.escaped));
            }
        }
        Some(())
    }

    fn escape(&mut self, in_class: bool, pending: &mut Vec<Pending>) -> Option<()> {
        let offset = self.offset();
        let index = self.position;
        let escaped = self.peek(1)?;
        let consumed = self.special_escape(escaped, in_class)?;
        if consumed > 0 {
            self.advance(consumed);
            return Some(());
        }
        self.advance(2);
        if self.unicode && !SYNTAX_CHARACTERS.contains(escaped) && !(in_class && escaped == '-') {
            return None;
        }
        if !in_class {
            if !GENERAL_ESCAPES.contains(escaped) && !OUTSIDE_CLASS_ESCAPES.contains(escaped) {
                self.found.push((offset, escaped));
            }
        } else if escaped == '^' || escaped == '-' {
            pending.push(Pending {
                offset,
                escaped,
                index,
            });
        } else if !GENERAL_ESCAPES.contains(escaped) {
            self.found.push((offset, escaped));
        }
        Some(())
    }

    fn special_escape(&self, escaped: char, in_class: bool) -> Option<usize> {
        let consumed = match escaped {
            character if CLASS_ESCAPES.contains(character) || CONTROL_ESCAPES.contains(character) => 2,
            'b' | 'B' if !in_class || escaped == 'b' => 2,
            '0'..='9' => 1_usize.saturating_add(self.digits_from(1)),
            'c' => {
                if self.peek(2).is_some_and(|letter| letter.is_ascii_alphabetic()) {
                    3
                } else {
                    1
                }
            }
            'x' => self.hex_escape(HEX_DIGITS_BYTE)?,
            'u' => self.unicode_escape()?,
            'p' | 'P' if self.unicode => self.braced_from(2)?,
            'k' if self.unicode || self.named_groups => self.named_reference()?,
            _ => 0,
        };
        Some(consumed)
    }

    fn digits_from(&self, start: usize) -> usize {
        self.count_while(start, |digit| digit.is_ascii_digit())
    }

    fn count_while(&self, start: usize, wanted: impl Fn(char) -> bool) -> usize {
        let from = self.position.saturating_add(start);
        self.characters
            .iter()
            .skip(from)
            .take_while(|(_, character)| wanted(*character))
            .count()
    }

    fn hex_escape(&self, digits: usize) -> Option<usize> {
        let valid = (2..digits.saturating_add(2))
            .all(|ahead| self.peek(ahead).is_some_and(|digit| digit.is_ascii_hexdigit()));
        match (valid, self.unicode) {
            (true, _) => Some(digits.saturating_add(2)),
            (false, true) => None,
            (false, false) => Some(0),
        }
    }

    fn unicode_escape(&self) -> Option<usize> {
        if self.unicode && self.peek(2) == Some('{') {
            return self.braced_from(2);
        }
        self.hex_escape(HEX_DIGITS_UNICODE)
    }

    fn braced_from(&self, start: usize) -> Option<usize> {
        if self.peek(start) != Some('{') {
            return None;
        }
        let length = self.count_while(start.saturating_add(1), |character| character != '}');
        self.peek(start.saturating_add(1).saturating_add(length))?;
        Some(start.saturating_add(length).saturating_add(2))
    }

    fn named_reference(&self) -> Option<usize> {
        if self.peek(2) != Some('<') {
            return None;
        }
        let length = self.count_while(3, |character| character != '>');
        self.peek(3_usize.saturating_add(length))?;
        Some(length.saturating_add(4))
    }
}
