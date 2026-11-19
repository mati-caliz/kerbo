const TAB_INDENT_WIDTH: usize = 8;
const SPACE: char = ' ';
const TAB: char = '\t';

// La sangría de una línea de docstring, como la mide ruff (y black): un tab llega hasta la
// próxima columna múltiplo de 8.
#[derive(Debug, Clone, Copy)]
pub enum Indentation {
    Spaces(usize),
    Tabs(usize),
    TabSpaces { tabs: usize, spaces: usize },
    SpacesTabs { spaces: usize, tabs: usize },
    Mixed { width: usize, length: usize },
}

const fn tab_offset(column: usize) -> usize {
    TAB_INDENT_WIDTH.saturating_sub(column % TAB_INDENT_WIDTH)
}

fn count_leading(text: &str, character: char) -> usize {
    text.chars().take_while(|current| *current == character).count()
}

fn mixed(rest: &str, mut width: usize, mut length: usize) -> Indentation {
    for character in rest.chars() {
        if character == TAB {
            width = width.saturating_add(tab_offset(width));
        } else if character.is_whitespace() {
            width = width.saturating_add(character.len_utf8());
        } else {
            break;
        }
        length = length.saturating_add(character.len_utf8());
    }
    Indentation::Mixed { width, length }
}

impl Indentation {
    pub fn of(line: &str) -> Self {
        let spaces = count_leading(line, SPACE);
        let after_spaces = line.get(spaces..).unwrap_or_default();
        let tabs = count_leading(after_spaces, TAB);
        if tabs == 0 {
            return Self::Spaces(spaces);
        }
        let after_tabs = after_spaces.get(tabs..).unwrap_or_default();
        let align_spaces = count_leading(after_tabs, SPACE);
        let rest = after_tabs.get(align_spaces..).unwrap_or_default();
        if spaces == 0 && align_spaces == 0 {
            return Self::Tabs(tabs);
        }
        if spaces == 0 && !rest.starts_with(TAB) {
            return Self::TabSpaces {
                tabs,
                spaces: align_spaces,
            };
        }
        if spaces > 0 && align_spaces == 0 {
            return Self::SpacesTabs { spaces, tabs };
        }
        let width = spaces
            .saturating_add(tabs.saturating_mul(TAB_INDENT_WIDTH))
            .saturating_add(align_spaces);
        let length = spaces.saturating_add(tabs).saturating_add(align_spaces);
        mixed(rest, width, length)
    }

    pub const fn columns(self) -> usize {
        match self {
            Self::Spaces(count) => count,
            Self::Tabs(count) => count.saturating_mul(TAB_INDENT_WIDTH),
            Self::TabSpaces { tabs, spaces } => tabs.saturating_mul(TAB_INDENT_WIDTH).saturating_add(spaces),
            Self::SpacesTabs { spaces, tabs } => spaces
                .saturating_add(tab_offset(spaces))
                .saturating_add(tabs.saturating_sub(1).saturating_mul(TAB_INDENT_WIDTH)),
            Self::Mixed { width, .. } => width,
        }
    }

    pub const fn text_len(self) -> usize {
        match self {
            Self::Spaces(count) | Self::Tabs(count) => count,
            Self::TabSpaces { tabs, spaces } | Self::SpacesTabs { spaces, tabs } => {
                tabs.saturating_add(spaces)
            }
            Self::Mixed { length, .. } => length,
        }
    }
}
