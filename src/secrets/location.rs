use std::ops::Range;

const NEWLINE: u8 = b'\n';
const CARRIAGE_RETURN: u8 = b'\r';
const FIRST_LINE: usize = 1;

#[derive(Debug)]
pub struct LineIndex {
    newlines: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Located {
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub text: Range<usize>,
}

impl LineIndex {
    pub fn new(content: &[u8]) -> Self {
        Self {
            newlines: content
                .iter()
                .enumerate()
                .filter_map(|(offset, &byte)| (byte == NEWLINE).then_some(offset))
                .collect(),
        }
    }

    pub fn locate(&self, content: &[u8], span: Range<usize>) -> Located {
        let before_start = self.newlines.partition_point(|&newline| newline <= span.start);
        let before_end = self
            .newlines
            .partition_point(|&newline| newline < span.end.saturating_sub(1));
        let line_start = before_start
            .checked_sub(1)
            .and_then(|previous| self.newlines.get(previous))
            .map_or(0, |&newline| newline.saturating_add(1));
        Located {
            line: before_start.saturating_add(FIRST_LINE),
            column: span.start.saturating_sub(line_start).saturating_add(1),
            end_line: before_end
                .saturating_add(FIRST_LINE)
                .max(before_start.saturating_add(FIRST_LINE)),
            text: self.reported_text(content, &span, before_start),
        }
    }

    fn reported_text(&self, content: &[u8], span: &Range<usize>, before_start: usize) -> Range<usize> {
        let Some(&line_end) = self.newlines.get(before_start) else {
            let rest = content.get(span.end..).unwrap_or_default();
            let stop = rest
                .iter()
                .position(|&byte| byte == NEWLINE || byte == CARRIAGE_RETURN)
                .unwrap_or(rest.len());
            return 0..span.end.saturating_add(stop);
        };
        let text_start = before_start
            .checked_sub(1)
            .and_then(|previous| self.newlines.get(previous))
            .copied()
            .unwrap_or(0);
        let end_line_end = self
            .newlines
            .iter()
            .find(|&&newline| newline >= span.end)
            .copied()
            .unwrap_or(line_end);
        text_start..end_line_end.max(line_end).max(span.end)
    }
}
