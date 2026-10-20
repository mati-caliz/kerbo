use tree_sitter::Point;

const OPENING: &[u8] = b"([{<";
const CLOSING: &[u8] = b")]}>";
const SEPARATOR: u8 = b',';
const ANNOTATION_MARK: u8 = b'@';
const LINE_FEED: u8 = b'\n';
const UNDERSCORE: u8 = b'_';
const DOLLAR: u8 = b'$';
const DOT: u8 = b'.';
const OPEN_PARENTHESIS: u8 = b'(';
const CLOSE_PARENTHESIS: u8 = b')';
const SLASH: u8 = b'/';
const ASTERISK: u8 = b'*';
const BLOCK_END: &[u8] = b"*/";
const QUOTE: u8 = b'"';
const APOSTROPHE: u8 = b'\'';
const BACKSLASH: u8 = b'\\';
const SPACE: u8 = b' ';
const RECEIVER: &str = "this";
const FINAL: &str = "final";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveredParameter {
    pub start: Point,
    pub type_start: Point,
    pub name: String,
    pub is_final: bool,
}

// tree-sitter-java no entiende una anotación de tipo antes de `...` (`Object @Nullable ... args`)
// y deja la lista de parámetros con un nodo de error; ahí se vuelve a partir la lista a mano.
pub fn parameters(original: &[u8], open: usize, close: usize) -> Vec<RecoveredParameter> {
    let masked = without_comments(original.get(..close).unwrap_or(original));
    let source = masked.as_slice();
    let inner_start = open.saturating_add(1);
    let mut pieces = Vec::new();
    let mut depth: usize = 0;
    let mut piece_start = inner_start;
    for offset in inner_start..close {
        let byte = source.get(offset).copied().unwrap_or_default();
        if OPENING.contains(&byte) {
            depth = depth.saturating_add(1);
        } else if CLOSING.contains(&byte) {
            depth = depth.saturating_sub(1);
        } else if byte == SEPARATOR && depth == 0 {
            pieces.push((piece_start, offset));
            piece_start = offset.saturating_add(1);
        }
    }
    pieces.push((piece_start, close));
    pieces
        .into_iter()
        .filter_map(|(start, end)| parameter(source, start, end))
        .collect()
}

fn parameter(source: &[u8], start: usize, end: usize) -> Option<RecoveredParameter> {
    let mut offset = skip_space(source, start, end);
    let first = offset;
    let mut is_final = false;
    let type_start = loop {
        if offset >= end {
            return None;
        }
        if source.get(offset) == Some(&ANNOTATION_MARK) {
            offset = skip_space(source, skip_annotation(source, offset, end), end);
            continue;
        }
        let word_end = word_end(source, offset, end);
        if source.get(offset..word_end) == Some(FINAL.as_bytes()) {
            is_final = true;
            offset = skip_space(source, word_end, end);
            continue;
        }
        break offset;
    };
    let name = last_word(source, type_start, end)?;
    (name != RECEIVER).then(|| RecoveredParameter {
        start: point(source, first),
        type_start: point(source, type_start),
        name: name.to_owned(),
        is_final,
    })
}

fn without_comments(source: &[u8]) -> Vec<u8> {
    let mut masked = source.to_vec();
    let mut offset = 0;
    while let Some(&byte) = source.get(offset) {
        let next = source.get(offset.saturating_add(1)).copied();
        let skipped_end = match (byte, next) {
            (SLASH, Some(SLASH)) => find(source, offset, &[LINE_FEED]),
            (SLASH, Some(ASTERISK)) => find(source, offset.saturating_add(2), BLOCK_END).saturating_add(2),
            (QUOTE | APOSTROPHE, _) => {
                offset = literal_end(source, offset, byte);
                continue;
            }
            _ => {
                offset = offset.saturating_add(1);
                continue;
            }
        };
        let end = skipped_end.min(source.len());
        for slot in masked.get_mut(offset..end).unwrap_or_default() {
            if *slot != LINE_FEED {
                *slot = SPACE;
            }
        }
        offset = end;
    }
    masked
}

fn find(source: &[u8], from: usize, needle: &[u8]) -> usize {
    source
        .get(from..)
        .and_then(|rest| rest.windows(needle.len()).position(|window| window == needle))
        .map_or(source.len(), |index| from.saturating_add(index))
}

fn literal_end(source: &[u8], start: usize, quote: u8) -> usize {
    let mut offset = start.saturating_add(1);
    while let Some(&byte) = source.get(offset) {
        offset = offset.saturating_add(if byte == BACKSLASH { 2 } else { 1 });
        if byte == quote || byte == LINE_FEED {
            break;
        }
    }
    offset
}

fn skip_space(source: &[u8], mut offset: usize, end: usize) -> usize {
    while offset < end && source.get(offset).is_some_and(u8::is_ascii_whitespace) {
        offset = offset.saturating_add(1);
    }
    offset
}

const fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == UNDERSCORE || byte == DOLLAR || !byte.is_ascii()
}

fn word_end(source: &[u8], mut offset: usize, end: usize) -> usize {
    while offset < end && source.get(offset).copied().is_some_and(is_word_byte) {
        offset = offset.saturating_add(1);
    }
    offset
}

fn skip_annotation(source: &[u8], at: usize, end: usize) -> usize {
    let mut offset = at.saturating_add(1);
    while offset < end
        && source
            .get(offset)
            .copied()
            .is_some_and(|byte| is_word_byte(byte) || byte == DOT)
    {
        offset = offset.saturating_add(1);
    }
    let after_name = skip_space(source, offset, end);
    if source.get(after_name) != Some(&OPEN_PARENTHESIS) {
        return offset;
    }
    let mut depth: usize = 0;
    for position in after_name..end {
        match source.get(position) {
            Some(&OPEN_PARENTHESIS) => depth = depth.saturating_add(1),
            Some(&CLOSE_PARENTHESIS) => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return position.saturating_add(1);
                }
            }
            _ => {}
        }
    }
    end
}

fn last_word(source: &[u8], start: usize, end: usize) -> Option<&str> {
    let bytes = source.get(start..end)?;
    let trimmed_end = bytes
        .iter()
        .rposition(|byte| !byte.is_ascii_whitespace())?
        .saturating_add(1);
    let word_start = bytes
        .get(..trimmed_end)?
        .iter()
        .rposition(|&byte| !is_word_byte(byte))
        .map_or(0, |index| index.saturating_add(1));
    let word = std::str::from_utf8(bytes.get(word_start..trimmed_end)?).ok()?;
    (!word.is_empty()).then_some(word)
}

fn point(source: &[u8], offset: usize) -> Point {
    let before = source.get(..offset).unwrap_or_default();
    let row = before.split(|&byte| byte == LINE_FEED).count().saturating_sub(1);
    let line_start = before
        .iter()
        .rposition(|&byte| byte == LINE_FEED)
        .map_or(0, |index| index.saturating_add(1));
    Point {
        row,
        column: offset.saturating_sub(line_start),
    }
}
