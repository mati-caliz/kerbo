use std::cell::Cell;
use std::rc::Rc;

use unicode_width::UnicodeWidthChar;

pub const INDENT_WIDTH: u32 = 4;
const TAB: char = '\t';
const NEWLINE: char = '\n';

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineMode {
    SoftOrSpace,
    Soft,
    Hard,
    Empty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrintMode {
    Flat,
    Expanded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupMode {
    Flat,
    Expand,
    Propagated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BestFittingMode {
    FirstLine,
    AllLines,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GroupId(usize);

impl GroupId {
    pub const fn new(index: usize) -> Self {
        Self(index)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Condition {
    pub mode: PrintMode,
    pub group: Option<GroupId>,
}

impl Condition {
    pub const fn if_group_breaks(group: GroupId) -> Self {
        Self {
            mode: PrintMode::Expanded,
            group: Some(group),
        }
    }

    pub const fn if_group_fits_on_line(group: GroupId) -> Self {
        Self {
            mode: PrintMode::Flat,
            group: Some(group),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagKind {
    Indent,
    Group,
    ConditionalGroup,
    ConditionalContent,
    IndentIfGroupBreaks,
    LineSuffix,
    FitsExpanded,
    BestFittingEntry,
    BestFitParenthesize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Start {
    Indent,
    Group {
        id: Option<GroupId>,
        mode: Cell<GroupMode>,
    },
    ConditionalGroup {
        condition: Condition,
        mode: Cell<GroupMode>,
    },
    ConditionalContent(Condition),
    IndentIfGroupBreaks(GroupId),
    LineSuffix {
        reserved_width: u32,
    },
    FitsExpanded {
        condition: Option<Condition>,
        propagate_expand: Cell<bool>,
    },
    BestFittingEntry,
    BestFitParenthesize {
        id: Option<GroupId>,
    },
}

impl Start {
    pub const fn kind(&self) -> TagKind {
        match self {
            Self::Indent => TagKind::Indent,
            Self::Group { .. } => TagKind::Group,
            Self::ConditionalGroup { .. } => TagKind::ConditionalGroup,
            Self::ConditionalContent(_) => TagKind::ConditionalContent,
            Self::IndentIfGroupBreaks(_) => TagKind::IndentIfGroupBreaks,
            Self::LineSuffix { .. } => TagKind::LineSuffix,
            Self::FitsExpanded { .. } => TagKind::FitsExpanded,
            Self::BestFittingEntry => TagKind::BestFittingEntry,
            Self::BestFitParenthesize { .. } => TagKind::BestFitParenthesize,
        }
    }
}

// El documento es la misma secuencia plana de ruff_formatter: el contenido de un grupo o de una
// sangría va entre su `Start` y su `End`. Cada variante de un `BestFitting` va entre un
// `Start(BestFittingEntry)` y su cierre, de la más plana a la más partida.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Element {
    Space,
    Line(LineMode),
    ExpandParent,
    LineSuffixBoundary,
    Token(&'static str),
    Text {
        text: Box<str>,
        width: Option<u32>,
    },
    Interned(Rc<[Self]>),
    BestFitting {
        variants: Box<[Box<[Self]>]>,
        mode: BestFittingMode,
    },
    Start(Start),
    End(TagKind),
}

pub fn char_width(character: char) -> u32 {
    if character == TAB {
        INDENT_WIDTH
    } else {
        clamped(character.width().unwrap_or(0))
    }
}

pub fn clamped(length: usize) -> u32 {
    u32::try_from(length).unwrap_or(u32::MAX)
}

// `None` es un texto de varias líneas: su ancho se mide carácter por carácter al imprimirlo.
pub fn text_width(text: &str) -> Option<u32> {
    if text.contains(NEWLINE) {
        return None;
    }
    Some(text.chars().fold(0_u32, |width, character| {
        width.saturating_add(char_width(character))
    }))
}
