use super::regex_code_units::CodeUnit;
use super::regex_syntax::Mode;

const CLASS_OPEN: u16 = 0x5b;
const CLASS_CLOSE: u16 = 0x5d;
const BACKSLASH: u16 = 0x5c;
const HYPHEN: u16 = 0x2d;
const AMPERSAND: u16 = 0x26;
const BRACE_OPEN: u16 = 0x7b;
const BRACE_CLOSE: u16 = 0x7d;
const FIRST_LEAD_SURROGATE: u32 = 0xd800;
const LAST_LEAD_SURROGATE: u32 = 0xdbff;
const FIRST_TAIL_SURROGATE: u32 = 0xdc00;
const LAST_TAIL_SURROGATE: u32 = 0xdfff;
const SUPPLEMENTARY_START: u32 = 0x1_0000;
const SURROGATE_BITS: u32 = 10;
const HEX_RADIX: u32 = 16;
const OCTAL_RADIX: u32 = 8;
const LARGEST_LEGACY_OCTAL: u32 = 0o377;
const CONTROL_MODULUS: u32 = 32;
const BYTE_ESCAPE_DIGITS: usize = 2;
const UNICODE_ESCAPE_DIGITS: usize = 4;
const MAX_OCTAL_DIGITS: usize = 3;

// Un `Character` de `regexpp`: el valor, dónde empieza y si se escribió `\u{...}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClassCharacter {
    pub value: u32,
    pub offset: usize,
    pub is_code_point_escape: bool,
}

#[derive(Debug, Clone, Copy)]
enum Atom {
    Character(ClassCharacter),
    Boundary,
}

fn is_lead(unit: u16) -> bool {
    (FIRST_LEAD_SURROGATE..=LAST_LEAD_SURROGATE).contains(&u32::from(unit))
}

fn is_tail(unit: u16) -> bool {
    (FIRST_TAIL_SURROGATE..=LAST_TAIL_SURROGATE).contains(&u32::from(unit))
}

pub const fn is_surrogate_pair(lead: u32, tail: u32) -> bool {
    lead >= FIRST_LEAD_SURROGATE
        && lead <= LAST_LEAD_SURROGATE
        && tail >= FIRST_TAIL_SURROGATE
        && tail <= LAST_TAIL_SURROGATE
}

fn combined(lead: u16, tail: u16) -> u32 {
    let high = u32::from(lead).saturating_sub(FIRST_LEAD_SURROGATE);
    let low = u32::from(tail).saturating_sub(FIRST_TAIL_SURROGATE);
    SUPPLEMENTARY_START
        .saturating_add(high << SURROGATE_BITS)
        .saturating_add(low)
}

// `iterateCharacterSequence` de ESLint sobre cada clase de un patrón ya validado: un rango corta la
// secuencia entre sus dos puntas, y un `\d`, una clase anidada o un `\q{}` la cortan del todo.
pub fn class_sequences(units: &[CodeUnit], mode: Mode) -> Vec<Vec<ClassCharacter>> {
    let mut scanner = Scanner {
        units,
        mode,
        index: 0,
        sequences: Vec::new(),
    };
    while let Some(unit) = scanner.peek() {
        scanner.index = scanner.index.saturating_add(1);
        if unit == BACKSLASH {
            scanner.index = scanner.index.saturating_add(1);
        } else if unit == CLASS_OPEN {
            scanner.class();
        }
    }
    scanner.sequences
}

struct Scanner<'units> {
    units: &'units [CodeUnit],
    mode: Mode,
    index: usize,
    sequences: Vec<Vec<ClassCharacter>>,
}

impl Scanner<'_> {
    fn peek(&self) -> Option<u16> {
        self.peek_at(0)
    }

    fn peek_at(&self, ahead: usize) -> Option<u16> {
        self.units
            .get(self.index.saturating_add(ahead))
            .map(|unit| unit.value)
    }

    const fn eat(&mut self, count: usize) {
        self.index = self.index.saturating_add(count);
    }

    fn eats_ascii(&mut self, wanted: u8) -> bool {
        let found = self.peek() == Some(u16::from(wanted));
        if found {
            self.eat(1);
        }
        found
    }

    fn class(&mut self) {
        self.eats_ascii(b'^');
        let mut finished = Vec::new();
        let mut current = Vec::new();
        let mut is_expression = false;
        while let Some(unit) = self.peek() {
            if unit == CLASS_CLOSE {
                self.eat(1);
                break;
            }
            if self.mode.unicode_sets && self.is_set_operator(unit) {
                self.eat(2);
                is_expression = true;
                continue;
            }
            self.element(&mut current, &mut finished);
        }
        finished.push(current);
        if !is_expression {
            self.sequences
                .extend(finished.into_iter().filter(|sequence| !sequence.is_empty()));
        }
    }

    fn is_set_operator(&self, unit: u16) -> bool {
        (unit == HYPHEN || unit == AMPERSAND) && self.peek_at(1) == Some(unit)
    }

    fn element(&mut self, current: &mut Vec<ClassCharacter>, finished: &mut Vec<Vec<ClassCharacter>>) {
        let Atom::Character(character) = self.atom() else {
            finished.push(std::mem::take(current));
            return;
        };
        current.push(character);
        if !self.starts_range() {
            return;
        }
        let hyphen_offset = self.units.get(self.index).map_or(0, |unit| unit.offset);
        self.eat(1);
        match self.atom() {
            Atom::Character(maximum) => {
                finished.push(std::mem::take(current));
                current.push(maximum);
            }
            Atom::Boundary => {
                current.push(ClassCharacter {
                    value: u32::from(HYPHEN),
                    offset: hyphen_offset,
                    is_code_point_escape: false,
                });
                finished.push(std::mem::take(current));
            }
        }
    }

    fn starts_range(&self) -> bool {
        self.peek() == Some(HYPHEN)
            && self.peek_at(1).is_some_and(|next| next != CLASS_CLOSE)
            && !(self.mode.unicode_sets && self.peek_at(1) == Some(HYPHEN))
    }

    fn atom(&mut self) -> Atom {
        let Some(unit) = self.units.get(self.index).copied() else {
            return Atom::Boundary;
        };
        self.eat(1);
        if unit.value == CLASS_OPEN && self.mode.unicode_sets {
            self.class();
            return Atom::Boundary;
        }
        if unit.value == BACKSLASH {
            return self.escape(unit.offset);
        }
        Atom::Character(self.plain(unit))
    }

    fn plain(&mut self, unit: CodeUnit) -> ClassCharacter {
        let tail = self
            .peek()
            .filter(|next| self.mode.is_strict() && is_lead(unit.value) && is_tail(*next));
        let value = tail.map_or_else(
            || u32::from(unit.value),
            |tail| {
                self.eat(1);
                combined(unit.value, tail)
            },
        );
        ClassCharacter {
            value,
            offset: unit.offset,
            is_code_point_escape: false,
        }
    }

    fn escape(&mut self, offset: usize) -> Atom {
        let Some(unit) = self.units.get(self.index).copied() else {
            return Atom::Boundary;
        };
        self.eat(1);
        let Ok(letter) = u8::try_from(unit.value) else {
            return Atom::Character(ClassCharacter {
                offset,
                ..self.plain(unit)
            });
        };
        let simple = |value: u32| {
            Atom::Character(ClassCharacter {
                value,
                offset,
                is_code_point_escape: false,
            })
        };
        match letter {
            b'd' | b'D' | b's' | b'S' | b'w' | b'W' => Atom::Boundary,
            b'p' | b'P' if self.mode.is_strict() => {
                self.skip_braces();
                Atom::Boundary
            }
            b'q' if self.mode.unicode_sets => {
                self.skip_braces();
                Atom::Boundary
            }
            b'n' => simple(0x0a),
            b'r' => simple(0x0d),
            b't' => simple(0x09),
            b'v' => simple(0x0b),
            b'f' => simple(0x0c),
            b'b' => simple(0x08),
            b'c' => self.control(offset),
            b'x' => simple(self.hex(BYTE_ESCAPE_DIGITS).unwrap_or_else(|| u32::from(letter))),
            b'u' => self.unicode(offset),
            b'0'..=b'7' if !self.mode.is_strict() || letter == b'0' => {
                self.index = self.index.saturating_sub(1);
                simple(self.octal())
            }
            _ => simple(u32::from(letter)),
        }
    }

    fn skip_braces(&mut self) {
        if self.peek() != Some(BRACE_OPEN) {
            return;
        }
        while let Some(unit) = self.peek() {
            self.eat(1);
            if unit == BACKSLASH {
                self.eat(1);
            } else if unit == BRACE_CLOSE {
                return;
            }
        }
    }

    fn control(&mut self, offset: usize) -> Atom {
        let value = self
            .peek()
            .and_then(|next| u8::try_from(next).ok())
            .filter(|next| {
                next.is_ascii_alphabetic()
                    || (!self.mode.is_strict() && (next.is_ascii_digit() || *next == b'_'))
            });
        let Some(control) = value else {
            self.index = self.index.saturating_sub(1);
            return Atom::Character(ClassCharacter {
                value: u32::from(BACKSLASH),
                offset,
                is_code_point_escape: false,
            });
        };
        self.eat(1);
        Atom::Character(ClassCharacter {
            value: u32::from(control) % CONTROL_MODULUS,
            offset,
            is_code_point_escape: false,
        })
    }

    fn hex(&mut self, digits: usize) -> Option<u32> {
        let mut value = 0_u32;
        for ahead in 0..digits {
            let digit = self
                .peek_at(ahead)
                .and_then(|unit| char::from_u32(u32::from(unit)))
                .and_then(|character| character.to_digit(HEX_RADIX))?;
            value = value.saturating_mul(HEX_RADIX).saturating_add(digit);
        }
        self.eat(digits);
        Some(value)
    }

    fn unicode(&mut self, offset: usize) -> Atom {
        let character = |value: u32, is_code_point_escape: bool| {
            Atom::Character(ClassCharacter {
                value,
                offset,
                is_code_point_escape,
            })
        };
        if self.mode.is_strict() && self.peek() == Some(BRACE_OPEN) {
            self.eat(1);
            let mut value = 0_u32;
            while let Some(digit) = self
                .peek()
                .and_then(|unit| char::from_u32(u32::from(unit)))
                .and_then(|found| found.to_digit(HEX_RADIX))
            {
                value = value.saturating_mul(HEX_RADIX).saturating_add(digit);
                self.eat(1);
            }
            self.eat(1);
            return character(value, true);
        }
        let Some(value) = self.hex(UNICODE_ESCAPE_DIGITS) else {
            return character(u32::from(b'u'), false);
        };
        let lead = u16::try_from(value).ok().filter(|unit| is_lead(*unit));
        if self.mode.is_strict()
            && let Some(lead) = lead
            && self.peek() == Some(BACKSLASH)
            && self.peek_at(1) == Some(u16::from(b'u'))
        {
            let checkpoint = self.index;
            self.eat(2);
            if let Some(tail) = self
                .hex(UNICODE_ESCAPE_DIGITS)
                .and_then(|tail| u16::try_from(tail).ok())
                && is_tail(tail)
            {
                return character(combined(lead, tail), false);
            }
            self.index = checkpoint;
        }
        character(value, false)
    }

    fn octal(&mut self) -> u32 {
        let mut value = 0_u32;
        for _ in 0..MAX_OCTAL_DIGITS {
            let Some(digit) = self
                .peek()
                .and_then(|unit| char::from_u32(u32::from(unit)))
                .and_then(|character| character.to_digit(OCTAL_RADIX))
            else {
                break;
            };
            let next = value.saturating_mul(OCTAL_RADIX).saturating_add(digit);
            if next > LARGEST_LEGACY_OCTAL {
                break;
            }
            value = next;
            self.eat(1);
        }
        value
    }
}
