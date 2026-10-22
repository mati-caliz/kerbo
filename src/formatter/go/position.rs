#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Pos(usize);

impl Pos {
    pub const NONE: Self = Self(0);

    pub const fn at(offset: usize) -> Self {
        Self(offset.saturating_add(1))
    }

    pub const fn is_valid(self) -> bool {
        self.0 > 0
    }

    pub const fn offset(self) -> Option<usize> {
        self.0.checked_sub(1)
    }

    pub const fn shifted(self, bytes: usize) -> Self {
        if self.is_valid() {
            Self(self.0.saturating_add(bytes))
        } else {
            Self::NONE
        }
    }

    pub const fn back(self, bytes: usize) -> Self {
        if self.is_valid() {
            Self(self.0.saturating_sub(bytes))
        } else {
            Self::NONE
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Position {
    pub offset: i64,
    pub line: i64,
    pub column: i64,
}

impl Position {
    pub const INFINITE: Self = Self {
        offset: INFINITY,
        line: INFINITY,
        column: 0,
    };

    pub const fn start() -> Self {
        Self {
            offset: 0,
            line: 1,
            column: 1,
        }
    }

    pub const fn is_valid(self) -> bool {
        self.line > 0
    }
}

pub const INFINITY: i64 = 1 << 30;

#[derive(Debug)]
pub struct LineTable {
    starts: Vec<usize>,
}

impl LineTable {
    pub fn new(source: &[u8]) -> Self {
        let breaks = source
            .iter()
            .enumerate()
            .filter(|(_, byte)| **byte == b'\n')
            .map(|(index, _)| index.saturating_add(1))
            .filter(|start| *start < source.len());
        Self {
            starts: std::iter::once(0).chain(breaks).collect(),
        }
    }

    pub fn position(&self, pos: Pos) -> Position {
        let Some(offset) = pos.offset() else {
            return Position::default();
        };
        let index = self
            .starts
            .partition_point(|start| *start <= offset)
            .saturating_sub(1);
        let line_start = self.starts.get(index).copied().unwrap_or_default();
        Position {
            offset: signed(offset),
            line: signed(index).saturating_add(1),
            column: signed(offset.saturating_sub(line_start)).saturating_add(1),
        }
    }

    pub fn line(&self, pos: Pos) -> i64 {
        self.position(pos).line
    }

    pub fn line_start(&self, line: i64) -> Pos {
        usize::try_from(line.saturating_sub(1))
            .ok()
            .and_then(|index| self.starts.get(index))
            .map_or(Pos::NONE, |start| Pos::at(*start))
    }

    pub fn line_count(&self) -> i64 {
        signed(self.starts.len())
    }
}

pub fn signed(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}
