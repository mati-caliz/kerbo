use super::{TextEdit, TextPoint, apply};

const LINE_FEED: char = '\n';
const LINE_ENDING: [char; 2] = ['\r', '\n'];
const FIRST_LINE: usize = 1;
const FIRST_COLUMN: usize = 1;
const NEWLINE: &str = "\n";
const REMOVED: &str = "-";
const ADDED: &str = "+";

pub fn unified(source: &[u8], edits: &[TextEdit]) -> Option<String> {
    let text = std::str::from_utf8(source).ok()?;
    let first = edits.iter().map(|edit| edit.start.line).min()?;
    let last = edits.iter().map(last_touched_line).max()?;
    let lines: Vec<&str> = text.split_inclusive(LINE_FEED).collect();
    let old = lines.get(first.checked_sub(FIRST_LINE)?..last.min(lines.len()))?;
    let shift = first.saturating_sub(FIRST_LINE);
    let relative: Vec<TextEdit> = edits
        .iter()
        .map(|edit| TextEdit {
            start: shifted(edit.start, shift),
            end: shifted(edit.end, shift),
            replacement: edit.replacement.clone(),
        })
        .collect();
    let replaced = apply(&relative, &old.concat())?;
    let new: Vec<&str> = replaced.split_inclusive(LINE_FEED).collect();
    let new_start = if new.is_empty() {
        first.saturating_sub(FIRST_LINE)
    } else {
        first
    };
    let mut hunk = vec![format!(
        "@@ -{} +{} @@",
        range(first, old.len()),
        range(new_start, new.len())
    )];
    hunk.extend(
        old.iter()
            .map(|line| [REMOVED, line.trim_end_matches(LINE_ENDING)].concat()),
    );
    hunk.extend(
        new.iter()
            .map(|line| [ADDED, line.trim_end_matches(LINE_ENDING)].concat()),
    );
    Some(hunk.join(NEWLINE))
}

const fn last_touched_line(edit: &TextEdit) -> usize {
    if edit.end.column == FIRST_COLUMN && edit.end.line > edit.start.line {
        edit.end.line.saturating_sub(FIRST_LINE)
    } else {
        edit.end.line
    }
}

const fn shifted(point: TextPoint, lines: usize) -> TextPoint {
    TextPoint {
        line: point.line.saturating_sub(lines),
        column: point.column,
    }
}

fn range(start: usize, count: usize) -> String {
    if count == 1 {
        start.to_string()
    } else {
        format!("{start},{count}")
    }
}
