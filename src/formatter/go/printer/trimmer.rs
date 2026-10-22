use super::ESCAPE;

const TAB: u8 = b'\t';
const VERTICAL_TAB: u8 = 0x0b;
const NEWLINE: u8 = b'\n';
const FORMFEED: u8 = 0x0c;
const SPACE: u8 = b' ';

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Space,
    Escape,
    Text,
}

// Port del `trimmer` de go/printer: saca los `ESCAPE` y los blancos al final de cada línea y pasa
// los saltos de página a saltos de línea.
#[derive(Debug)]
pub struct Trimmer {
    state: State,
    space: Vec<u8>,
    output: Vec<u8>,
}

impl Trimmer {
    pub const fn new() -> Self {
        Self {
            state: State::Space,
            space: Vec::new(),
            output: Vec::new(),
        }
    }

    pub fn into_output(self) -> Vec<u8> {
        self.output
    }

    fn in_escape(&mut self, data: &[u8], (mark, index): (usize, usize), byte: u8) {
        if byte == ESCAPE {
            self.output
                .extend_from_slice(data.get(mark..index).unwrap_or_default());
            self.reset_space();
        }
    }

    fn reset_space(&mut self) {
        self.state = State::Space;
        self.space.clear();
    }

    pub fn write(&mut self, data: &[u8]) {
        let mut mark = 0;
        for (index, original) in data.iter().copied().enumerate() {
            let byte = if original == VERTICAL_TAB { TAB } else { original };
            match self.state {
                State::Space => mark = self.in_space(index, byte).unwrap_or(mark),
                State::Escape => self.in_escape(data, (mark, index), byte),
                State::Text => mark = self.in_text(data, (mark, index), byte),
            }
        }
        if self.state != State::Space {
            self.output
                .extend_from_slice(data.get(mark..).unwrap_or_default());
            self.reset_space();
        }
    }

    fn in_space(&mut self, index: usize, byte: u8) -> Option<usize> {
        match byte {
            TAB | SPACE => {
                self.space.push(byte);
                None
            }
            NEWLINE | FORMFEED => {
                self.reset_space();
                self.output.push(NEWLINE);
                None
            }
            ESCAPE => {
                self.output.append(&mut self.space);
                self.state = State::Escape;
                Some(index.saturating_add(1))
            }
            _ => {
                self.output.append(&mut self.space);
                self.state = State::Text;
                Some(index)
            }
        }
    }

    fn in_text(&mut self, data: &[u8], (mark, index): (usize, usize), byte: u8) -> usize {
        match byte {
            TAB | SPACE => {
                self.output
                    .extend_from_slice(data.get(mark..index).unwrap_or_default());
                self.reset_space();
                self.space.push(byte);
                mark
            }
            NEWLINE | FORMFEED => {
                self.output
                    .extend_from_slice(data.get(mark..index).unwrap_or_default());
                self.reset_space();
                self.output.push(NEWLINE);
                mark
            }
            ESCAPE => {
                self.output
                    .extend_from_slice(data.get(mark..index).unwrap_or_default());
                self.state = State::Escape;
                index.saturating_add(1)
            }
            _ => mark,
        }
    }
}

pub fn trim(chunks: &[&[u8]]) -> Vec<u8> {
    let mut trimmer = Trimmer::new();
    for chunk in chunks {
        trimmer.write(chunk);
    }
    trimmer.output
}
