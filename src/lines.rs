use tree_sitter::Node;

use crate::profile::{CountedLines, LineSpan, Profile, SpanBounds};

#[derive(Debug)]
pub struct LineMap {
    code_rows: Vec<bool>,
    blank_rows: Vec<bool>,
}

impl LineMap {
    pub fn new(root: Node<'_>, source: &[u8], profile: &Profile) -> Self {
        let blank_rows = blank_rows(source);
        let mut code_rows = vec![false; blank_rows.len()];
        let mut pending = vec![root];
        while let Some(node) = pending.pop() {
            if profile.comments.contains(&node.kind()) {
                continue;
            }
            if node.child_count() == 0 || profile.literals.contains(&node.kind()) {
                mark_rows(&mut code_rows, node.start_position().row, node.end_position().row);
                continue;
            }
            let mut cursor = node.walk();
            pending.extend(node.children(&mut cursor));
        }
        Self {
            code_rows,
            blank_rows,
        }
    }

    pub const fn total_rows(&self) -> usize {
        self.code_rows.len()
    }

    pub fn count(&self, first_row: usize, last_row: usize, span: LineSpan) -> usize {
        let (first, last) = if span.bounds == SpanBounds::Interior {
            (first_row.saturating_add(1), last_row.saturating_sub(1))
        } else {
            (first_row, last_row)
        };
        if first > last {
            return 0;
        }
        (first..=last)
            .filter(|&row| self.is_counted(row, span.counted))
            .count()
    }

    fn is_counted(&self, row: usize, counted: CountedLines) -> bool {
        match counted {
            CountedLines::All => true,
            CountedLines::Tokens => self.code_rows.get(row).copied().unwrap_or(false),
            CountedLines::Code => {
                let blank = self.blank_rows.get(row).copied().unwrap_or(true);
                !blank && self.code_rows.get(row).copied().unwrap_or(false)
            }
        }
    }
}

fn blank_rows(source: &[u8]) -> Vec<bool> {
    if source.is_empty() {
        return Vec::new();
    }
    let without_final_newline = source.strip_suffix(b"\n").unwrap_or(source);
    without_final_newline
        .split(|&byte| byte == b'\n')
        .map(|row| row.trim_ascii().is_empty())
        .collect()
}

#[cfg(test)]
fn row_count(source: &[u8]) -> usize {
    if source.is_empty() {
        return 0;
    }
    let without_final_newline = source.strip_suffix(b"\n").unwrap_or(source);
    without_final_newline.split(|&byte| byte == b'\n').count()
}

fn mark_rows(rows: &mut [bool], first: usize, last: usize) {
    for row in rows.iter_mut().take(last.saturating_add(1)).skip(first) {
        *row = true;
    }
}

#[cfg(test)]
mod tests {
    use super::row_count;

    #[test]
    fn counts_rows_like_editors_do() {
        assert_eq!(row_count(b""), 0);
        assert_eq!(row_count(b"\n"), 1);
        assert_eq!(row_count(b"a"), 1);
        assert_eq!(row_count(b"a\n"), 1);
        assert_eq!(row_count(b"a\nb"), 2);
        assert_eq!(row_count(b"a\n\nb\n"), 3);
    }
}
