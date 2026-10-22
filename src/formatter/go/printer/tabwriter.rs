use super::ESCAPE;
use super::trimmer::Trimmer;

const TAB_WIDTH: usize = 8;
const PADDING: usize = 1;
const ESCAPE_WIDTH: usize = 2;
const TAB: u8 = b'\t';
const VERTICAL_TAB: u8 = 0x0b;
const NEWLINE: u8 = b'\n';
const FORMFEED: u8 = 0x0c;
const PADDING_BYTE: u8 = b' ';
const UTF8_CONTINUATION_MASK: u8 = 0xc0;
const UTF8_CONTINUATION: u8 = 0x80;

#[derive(Debug, Clone, Copy, Default)]
struct Cell {
    size: usize,
    width: usize,
    htab: bool,
}

// Port de `text/tabwriter` con la configuración de gofmt: ancho mínimo 0, tabulador de 8, relleno
// de un espacio, columnas vacías descartadas e indentación con tabs.
#[derive(Debug)]
struct TabWriter {
    buf: Vec<u8>,
    pos: usize,
    cell: Cell,
    escaping: bool,
    lines: Vec<Vec<Cell>>,
    widths: Vec<usize>,
    sink: Trimmer,
}

pub fn align(text: &[u8]) -> Vec<u8> {
    let mut writer = TabWriter {
        buf: Vec::new(),
        pos: 0,
        cell: Cell::default(),
        escaping: false,
        lines: vec![Vec::new()],
        widths: Vec::new(),
        sink: Trimmer::new(),
    };
    writer.write(text);
    writer.flush();
    writer.sink.output
}

fn rune_count(bytes: &[u8]) -> usize {
    bytes
        .iter()
        .filter(|byte| **byte & UTF8_CONTINUATION_MASK != UTF8_CONTINUATION)
        .count()
}

impl TabWriter {
    fn reset(&mut self) {
        self.buf.clear();
        self.pos = 0;
        self.cell = Cell::default();
        self.escaping = false;
        self.lines.clear();
        self.lines.push(Vec::new());
        self.widths.clear();
    }

    fn append(&mut self, text: &[u8]) {
        self.buf.extend_from_slice(text);
        self.cell.size = self.cell.size.saturating_add(text.len());
    }

    fn update_width(&mut self) {
        let counted = rune_count(self.buf.get(self.pos..).unwrap_or_default());
        self.cell.width = self.cell.width.saturating_add(counted);
        self.pos = self.buf.len();
    }

    fn end_escape(&mut self) {
        self.update_width();
        self.cell.width = self.cell.width.saturating_sub(ESCAPE_WIDTH);
        self.pos = self.buf.len();
        self.escaping = false;
    }

    fn terminate_cell(&mut self, htab: bool) -> usize {
        self.cell.htab = htab;
        let cell = std::mem::take(&mut self.cell);
        let Some(line) = self.lines.last_mut() else {
            return 0;
        };
        line.push(cell);
        line.len()
    }

    fn write(&mut self, text: &[u8]) {
        let mut start = 0;
        for (index, byte) in text.iter().copied().enumerate() {
            if self.escaping {
                if byte == ESCAPE {
                    self.append(text.get(start..=index).unwrap_or_default());
                    start = index.saturating_add(1);
                    self.end_escape();
                }
                continue;
            }
            if matches!(byte, TAB | VERTICAL_TAB | NEWLINE | FORMFEED) {
                self.append(text.get(start..index).unwrap_or_default());
                self.update_width();
                start = index.saturating_add(1);
                let cells = self.terminate_cell(byte == TAB);
                if byte == NEWLINE || byte == FORMFEED {
                    self.lines.push(Vec::new());
                    if byte == FORMFEED || cells == 1 {
                        self.flush();
                    }
                }
            } else if byte == ESCAPE {
                self.append(text.get(start..index).unwrap_or_default());
                self.update_width();
                start = index;
                self.escaping = true;
            }
        }
        self.append(text.get(start..).unwrap_or_default());
    }

    fn flush(&mut self) {
        if self.cell.size > 0 {
            if self.escaping {
                self.end_escape();
            }
            self.terminate_cell(false);
        }
        let count = self.lines.len();
        self.format(0, 0, count);
        self.reset();
    }

    fn format(&mut self, pos: usize, first_line: usize, end_line: usize) -> usize {
        let mut pos = pos;
        let mut first = first_line;
        let column = self.widths.len();
        let mut this = first_line;
        while this < end_line {
            if self.cell_count(this) <= column.saturating_add(1) {
                this = this.saturating_add(1);
                continue;
            }
            pos = self.write_lines(pos, first, this);
            first = this;
            let mut width = 0;
            let mut discardable = true;
            while this < end_line && self.cell_count(this) > column.saturating_add(1) {
                let cell = self.cell_at(this, column);
                width = width.max(cell.width.saturating_add(PADDING));
                if cell.width > 0 || cell.htab {
                    discardable = false;
                }
                this = this.saturating_add(1);
            }
            if discardable {
                width = 0;
            }
            self.widths.push(width);
            pos = self.format(pos, first, this);
            self.widths.pop();
            first = this;
        }
        self.write_lines(pos, first, end_line)
    }

    fn cell_count(&self, line: usize) -> usize {
        self.lines.get(line).map_or(0, Vec::len)
    }

    fn cell_at(&self, line: usize, column: usize) -> Cell {
        self.lines
            .get(line)
            .and_then(|cells| cells.get(column))
            .copied()
            .unwrap_or_default()
    }

    fn write_lines(&mut self, pos: usize, first_line: usize, end_line: usize) -> usize {
        let mut pos = pos;
        for line in first_line..end_line {
            let cells = self.lines.get(line).cloned().unwrap_or_default();
            let mut use_tabs = true;
            for (column, cell) in cells.iter().enumerate() {
                let column_width = self.widths.get(column).copied();
                if cell.size == 0 {
                    if let Some(width) = column_width {
                        self.write_padding(cell.width, width, use_tabs);
                    }
                    continue;
                }
                use_tabs = false;
                let end = pos.saturating_add(cell.size);
                let text = self.buf.get(pos..end).unwrap_or_default().to_vec();
                self.sink.write(&text);
                pos = end;
                if let Some(width) = column_width {
                    self.write_padding(cell.width, width, false);
                }
            }
            if line.saturating_add(1) == self.lines.len() {
                let end = pos.saturating_add(self.cell.size);
                let text = self.buf.get(pos..end).unwrap_or_default().to_vec();
                self.sink.write(&text);
                pos = end;
            } else {
                self.sink.write(&[NEWLINE]);
            }
        }
        pos
    }

    fn write_padding(&mut self, text_width: usize, cell_width: usize, use_tabs: bool) {
        if use_tabs {
            let rounded = cell_width.div_ceil(TAB_WIDTH).saturating_mul(TAB_WIDTH);
            let tabs = rounded.saturating_sub(text_width).div_ceil(TAB_WIDTH);
            self.sink.write(&vec![TAB; tabs]);
            return;
        }
        self.sink
            .write(&vec![PADDING_BYTE; cell_width.saturating_sub(text_width)]);
    }
}
