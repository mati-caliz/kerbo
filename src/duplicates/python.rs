use super::python_letters::is_letter;
use super::{CpdToken, SuppressionState, image_hash, suppression};

const BYTE_ORDER_MARK: char = '\u{feff}';
const FORM_FEED: char = '\u{c}';
const SINGLE_STRING_CONTINUATIONS: [&str; 2] = ["\\\r\n", "\\\n"];
const OPERATORS: &[&str] = &[
    "**=", "//=", "<<=", ">>=", "//", "**", "<<", ">>", "==", "<=", ">=", "<>", "!=", "+=", "-=", "*=", "/=",
    "%=", "&=", "|=", "^=", "(", ")", "{", "}", "[", "]", ";", ",", ".", ":", "`", "+", "-", "*", "/", "%",
    "~", "^", "|", "&", "=", ">", "<", "@",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StringOpening {
    length: usize,
    quote: char,
    triple: bool,
}

struct Lexer {
    characters: Vec<char>,
    lines: Vec<usize>,
    position: usize,
    comments: Vec<String>,
    suppressing: SuppressionState,
    found: Vec<CpdToken>,
}

pub fn tokens(source: &[u8]) -> Option<Vec<CpdToken>> {
    let text = String::from_utf8_lossy(source);
    let characters: Vec<char> = text
        .strip_prefix(BYTE_ORDER_MARK)
        .unwrap_or(&text)
        .chars()
        .collect();
    let mut lexer = Lexer {
        lines: line_numbers(&characters),
        characters,
        position: 0,
        comments: Vec::new(),
        suppressing: SuppressionState::default(),
        found: Vec::new(),
    };
    lexer.run()?;
    Some(lexer.found)
}

fn line_numbers(characters: &[char]) -> Vec<usize> {
    let mut line = 1_usize;
    let mut lines = Vec::with_capacity(characters.len());
    let mut following = characters.iter().skip(1);
    for &character in characters {
        lines.push(line);
        let next = following.next().copied();
        if character == '\n' || (character == '\r' && next != Some('\n')) {
            line = line.saturating_add(1);
        }
    }
    lines
}

impl Lexer {
    fn at(&self, offset: usize) -> Option<char> {
        self.characters.get(self.position.saturating_add(offset)).copied()
    }

    fn starts_with(&self, text: &str) -> bool {
        text.chars()
            .enumerate()
            .all(|(offset, expected)| self.at(offset) == Some(expected))
    }

    fn slice(&self, start: usize, end: usize) -> String {
        self.characters
            .get(start..end)
            .unwrap_or_default()
            .iter()
            .collect()
    }

    fn run(&mut self) -> Option<()> {
        while let Some(current) = self.at(0) {
            match current {
                ' ' | '\t' | '\n' | '\r' | FORM_FEED => self.position = self.position.saturating_add(1),
                '\\' => self.skip_continuation()?,
                '#' => self.read_comment(),
                _ => self.read_token()?,
            }
        }
        Some(())
    }

    fn skip_continuation(&mut self) -> Option<()> {
        let length = if self.starts_with("\\\r\n") {
            3
        } else if self.starts_with("\\\n") || self.starts_with("\\\r") {
            2
        } else {
            return None;
        };
        self.position = self.position.saturating_add(length);
        Some(())
    }

    fn read_comment(&mut self) {
        let start = self.position;
        while self
            .at(0)
            .is_some_and(|character| character != '\n' && character != '\r')
        {
            self.position = self.position.saturating_add(1);
        }
        self.comments.push(self.slice(start, self.position));
    }

    fn read_token(&mut self) -> Option<()> {
        let start = self.position;
        let opening = self.string_opening();
        let plain = [self.operator_length(), self.name_length(), self.number_length()]
            .into_iter()
            .max()
            .unwrap_or_default();
        let image = match opening.filter(|opening| opening.length > plain) {
            Some(opening) => self.read_string(opening)?,
            None if plain > 0 => {
                self.position = start.saturating_add(plain);
                self.slice(start, self.position)
            }
            None => return None,
        };
        let leading = suppression(self.comments.iter().map(String::as_str));
        self.comments.clear();
        if self.suppressing.apply(leading) {
            return Some(());
        }
        self.found.push(CpdToken {
            image: image_hash(&image),
            line: self.lines.get(start).copied().unwrap_or(1),
            end_line: self
                .lines
                .get(self.position.saturating_sub(1))
                .copied()
                .unwrap_or(1),
        });
        Some(())
    }

    fn operator_length(&self) -> usize {
        OPERATORS
            .iter()
            .find(|operator| self.starts_with(operator))
            .map_or(0, |operator| operator.len())
    }

    fn name_length(&self) -> usize {
        if !self.at(0).is_some_and(is_letter) {
            return 0;
        }
        self.run_end(1, |character| is_letter(character) || character.is_ascii_digit())
    }

    fn string_opening(&self) -> Option<StringOpening> {
        let first = self.at(0)?;
        let prefix = match first.to_ascii_lowercase() {
            'u' | 'b' => 1_usize.saturating_add(usize::from(
                self.at(1).is_some_and(|raw| raw.eq_ignore_ascii_case(&'r')),
            )),
            'r' => 1,
            _ => 0,
        };
        let quote = self.at(prefix).filter(|&quote| quote == '\'' || quote == '"')?;
        let triple = self.at(prefix.saturating_add(1)) == Some(quote)
            && self.at(prefix.saturating_add(2)) == Some(quote);
        Some(StringOpening {
            length: prefix.saturating_add(if triple { 3 } else { 1 }),
            quote,
            triple,
        })
    }

    fn read_string(&mut self, opening: StringOpening) -> Option<String> {
        let start = self.position;
        self.position = self.position.saturating_add(opening.length);
        if opening.triple {
            let body = self.read_triple_body(opening.quote)?;
            return Some(format!(
                "{}{body}",
                self.slice(start, start.saturating_add(opening.length))
            ));
        }
        self.read_single_body(opening.quote)?;
        let raw = self.slice(start, self.position);
        Some(
            SINGLE_STRING_CONTINUATIONS
                .iter()
                .fold(raw, |image, continuation| image.replace(continuation, "")),
        )
    }

    fn read_single_body(&mut self, quote: char) -> Option<()> {
        loop {
            let current = self.at(0)?;
            let length = match (current, self.at(1)) {
                (character, _) if character == quote => {
                    self.position = self.position.saturating_add(1);
                    return Some(());
                }
                ('\\', Some('\r')) if self.at(2) == Some('\n') => 3,
                ('\\', Some(next)) if next == '\n' || next == '\r' || next == '\\' || next == quote => 2,
                ('\n' | '\r', _) => return None,
                _ => 1,
            };
            self.position = self.position.saturating_add(length);
        }
    }

    fn read_triple_body(&mut self, quote: char) -> Option<String> {
        let closing: String = [quote; 3].iter().collect();
        let mut body = String::new();
        loop {
            if self.starts_with(&closing) {
                self.position = self.position.saturating_add(3);
                body.push_str(&closing);
                return Some(body);
            }
            let current = self.at(0)?;
            let next = self.at(1);
            let (text, length) = match (current, next) {
                ('\r', Some('\n')) => (String::from('\n'), 2),
                ('\n' | '\r', _) => (String::from('\n'), 1),
                ('\\', Some(escaped)) if escaped != '\n' && escaped != '\r' => {
                    ([current, escaped].iter().collect(), 2)
                }
                _ => (String::from(current), 1),
            };
            body.push_str(&text);
            self.position = self.position.saturating_add(length);
        }
    }

    fn number_length(&self) -> usize {
        let decimal = self.decimal_end(0);
        let floats = self.float_ends();
        let zero_decimal = (self.at(0) == Some('0')).then(|| self.decimal_end(1)).flatten();
        let complex = decimal
            .into_iter()
            .chain(floats.iter().copied())
            .chain(zero_decimal)
            .filter(|&end| {
                self.at(end)
                    .is_some_and(|suffix| suffix.eq_ignore_ascii_case(&'j'))
            })
            .map(|end| end.saturating_add(1))
            .collect::<Vec<_>>();
        let radix = [('x', 16), ('o', 8), ('b', 2)]
            .into_iter()
            .filter_map(|(marker, radix)| self.radix_end(marker, radix));
        decimal
            .into_iter()
            .chain(floats)
            .chain(complex)
            .chain(radix)
            .max()
            .unwrap_or_default()
    }

    fn digits_end(&self, start: usize, radix: u32) -> usize {
        self.run_end(start, |character| character.is_digit(radix))
    }

    fn run_end(&self, start: usize, continues: impl Fn(char) -> bool) -> usize {
        let remaining = self.characters.len().saturating_sub(self.position);
        (start..=remaining.max(start))
            .find(|&offset| !self.at(offset).is_some_and(&continues))
            .unwrap_or(start)
    }

    fn long_suffix(&self, end: usize) -> usize {
        end.saturating_add(usize::from(
            self.at(end)
                .is_some_and(|suffix| suffix.eq_ignore_ascii_case(&'l')),
        ))
    }

    fn decimal_end(&self, start: usize) -> Option<usize> {
        match self.at(start)? {
            '0' => Some(start.saturating_add(1)),
            '1'..='9' => Some(self.long_suffix(self.digits_end(start.saturating_add(1), 10))),
            _ => None,
        }
    }

    fn radix_end(&self, marker: char, radix: u32) -> Option<usize> {
        if self.at(0) != Some('0') {
            return None;
        }
        let has_radix_marker = self
            .at(1)
            .is_some_and(|found| found.eq_ignore_ascii_case(&marker));
        let digits_start = 1_usize.saturating_add(usize::from(has_radix_marker));
        let digits = self.digits_end(digits_start, radix);
        if radix == 16 && digits == digits_start {
            return None;
        }
        if radix == 16 && !has_radix_marker {
            return None;
        }
        Some(self.long_suffix(digits))
    }

    fn exponent_end(&self, start: usize) -> Option<usize> {
        if !self
            .at(start)
            .is_some_and(|marker| marker.eq_ignore_ascii_case(&'e'))
        {
            return None;
        }
        let signed = start.saturating_add(1).saturating_add(usize::from(matches!(
            self.at(start.saturating_add(1)),
            Some('+' | '-')
        )));
        let end = self.digits_end(signed, 10);
        (end > signed).then_some(end)
    }

    fn float_ends(&self) -> Vec<usize> {
        let integer = self.digits_end(0, 10);
        let mut ends = Vec::new();
        if integer > 0 && self.at(integer) == Some('.') {
            let fraction = self.digits_end(integer.saturating_add(1), 10);
            ends.push(self.exponent_end(fraction).unwrap_or(fraction));
        }
        if self.at(0) == Some('.') {
            let fraction = self.digits_end(1, 10);
            if fraction > 1 {
                ends.push(self.exponent_end(fraction).unwrap_or(fraction));
            }
        }
        if integer > 0 {
            ends.extend(self.exponent_end(integer));
        }
        ends
    }
}
