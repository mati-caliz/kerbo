use super::blocks::LinkDefinition;

const OPENING_QUOTES: &str = "``";
const CLOSING_QUOTES: &str = "''";
const OPENING_CURLY_QUOTE: char = '“';
const CLOSING_CURLY_QUOTE: char = '”';
const BACKTICK: u8 = b'`';
const CODE_FENCE_LENGTH: usize = 3;
const SCHEME_SEPARATOR: &[u8] = b"://";
const SCHEMES: &[&str] = &["file", "ftp", "gopher", "http", "https", "mailto", "nntp"];
const SHORTEST_SCHEME: usize = 3;
const LONGEST_SCHEME: usize = 6;
const SHORTEST_URL: usize = 7;
const HOST_SYMBOLS: &[u8] = b"_@-.[]:";
const URL_PUNCTUATION: &[u8] = b".,:;?!";
const PATH_SYMBOLS: &[u8] = b"$'()*+&#=@~_/-[]{}%";

pub fn is_scheme(scheme: &str) -> bool {
    SCHEMES.contains(&scheme)
}

fn is_host(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || HOST_SYMBOLS.contains(&byte)
}

fn is_url_punctuation(byte: u8) -> bool {
    URL_PUNCTUATION.contains(&byte)
}

fn is_path(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || PATH_SYMBOLS.contains(&byte)
}

const fn closing_bracket(byte: u8) -> Option<u8> {
    match byte {
        b'(' => Some(b')'),
        b'{' => Some(b'}'),
        b'[' => Some(b']'),
        _ => None,
    }
}

fn path_end(bytes: &[u8], start: usize) -> usize {
    let mut pending_closers = Vec::new();
    let mut end = start;
    for (index, &byte) in bytes.iter().enumerate().skip(start) {
        if is_url_punctuation(byte) {
            continue;
        }
        if !is_path(byte) {
            break;
        }
        if let Some(closer) = closing_bracket(byte) {
            pending_closers.push(closer);
        } else if matches!(byte, b')' | b'}' | b']') && pending_closers.pop() != Some(byte) {
            break;
        }
        if pending_closers.is_empty() {
            end = index + 1;
        }
    }
    end
}

fn auto_url_length(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    if bytes.len() < SHORTEST_URL {
        return None;
    }
    let colon = (SHORTEST_SCHEME..=LONGEST_SCHEME).find(|&index| bytes.get(index) == Some(&b':'))?;
    let host_start = colon + SCHEME_SEPARATOR.len();
    if bytes.get(colon..host_start) != Some(SCHEME_SEPARATOR) || !is_scheme(text.get(..colon)?) {
        return None;
    }
    let first = *bytes.get(host_start)?;
    if !is_host(first) || is_url_punctuation(first) {
        return None;
    }
    let mut host_end = host_start + 1;
    for (index, &byte) in bytes.iter().enumerate().skip(host_start + 1) {
        if !is_host(byte) {
            break;
        }
        if !is_url_punctuation(byte) {
            host_end = index + 1;
        }
    }
    Some(path_end(bytes, host_end))
}

fn identifier_length(text: &str) -> usize {
    let mut length = 0;
    for character in text.chars() {
        let continues = if character.is_ascii() {
            (character.is_ascii_alphanumeric() || character == '_')
                && (length > 0 || !character.is_ascii_digit())
        } else {
            character.is_alphabetic()
        };
        if !continues {
            break;
        }
        length += character.len_utf8();
    }
    length
}

fn skipped_by_auto_link(rest: &str) -> usize {
    auto_url_length(rest).unwrap_or_else(|| identifier_length(rest))
}

// `parseText` de go/doc/comment visto desde la salida: el texto queda igual salvo `` y '' que pasan
// a comillas curvas, fuera de las URLs y de los identificadores. Tras tres o más acentos graves, el
// original avanza comparando el índice del texto entero contra el resto, y se copia así.
fn with_curly_quotes(text: &str, auto_link: bool) -> String {
    let mut out = String::new();
    let mut index = 0;
    let mut written = 0;
    while let Some(rest) = text.get(index..).filter(|rest| !rest.is_empty()) {
        let skipped = if auto_link { skipped_by_auto_link(rest) } else { 0 };
        let quote = if rest.starts_with(OPENING_QUOTES) {
            Some(OPENING_CURLY_QUOTE)
        } else if rest.starts_with(CLOSING_QUOTES) {
            Some(CLOSING_CURLY_QUOTE)
        } else {
            None
        };
        if skipped > 0 {
            index += skipped;
        } else if rest.as_bytes().get(CODE_FENCE_LENGTH - 1) == Some(&BACKTICK)
            && rest.starts_with(OPENING_QUOTES)
        {
            index += CODE_FENCE_LENGTH;
            while rest.as_bytes().get(index) == Some(&BACKTICK) {
                index += 1;
            }
        } else if let Some(curly) = quote {
            out.push_str(text.get(written..index).unwrap_or_default());
            out.push(curly);
            index += OPENING_QUOTES.len();
            written = index;
        } else {
            index += rest.chars().next().map_or(1, char::len_utf8);
        }
    }
    out.push_str(text.get(written..).unwrap_or_default());
    out
}

const fn as_link_text(character: char) -> char {
    if matches!(character, '\n' | '\t') {
        ' '
    } else {
        character
    }
}

// `parseLinkedText`: un `[texto]` con definición de enlace la marca como usada. El texto buscado se
// junta desde el primer `[` sin cerrar, como en el original, aunque después haya otro `[`.
pub fn linked_text(text: &str, links: &mut [LinkDefinition]) -> String {
    let mut out = String::new();
    let mut written = 0;
    let mut open: Option<usize> = None;
    let mut wanted = String::new();
    for (index, character) in text.char_indices() {
        if character == '[' {
            open = Some(index);
            continue;
        }
        if character != ']' {
            wanted.extend(open.map(|_| as_link_text(character)));
            continue;
        }
        let definition = links.iter_mut().find(|definition| definition.text == wanted);
        if let Some(start) = open
            && let Some(definition) = definition
        {
            definition.used = true;
            out.push_str(&with_curly_quotes(
                text.get(written..start).unwrap_or_default(),
                true,
            ));
            out.push('[');
            out.push_str(&with_curly_quotes(
                text.get(start + 1..index).unwrap_or_default(),
                false,
            ));
            out.push(']');
            written = index + 1;
        }
        open = None;
        wanted.clear();
    }
    out.push_str(&with_curly_quotes(text.get(written..).unwrap_or_default(), true));
    out
}
