use tree_sitter::Node;

use crate::shell::parameters::{is_array_expansion, reference};
use crate::shell::syntax::{
    ANSI_STRING, CONCATENATION, EXPANSION, NUMBER, RAW_STRING, SIMPLE_EXPANSION, STRING, STRING_CONTENT,
    WORD, children, named_children, text,
};
use crate::shell::words::{braced_content, is_brace_expansion, literal, literal_or, parts};

const GLOB_CHARACTERS: &[u8] = b"*?";
const OPEN_CLASS: u8 = b'[';
const CLOSE_CLASS: u8 = b']';
const ESCAPE: u8 = b'\\';
const EXTGLOB_STARTS: &[&str] = &["?(", "*(", "+(", "@(", "!("];
const INDIRECTION: &str = "!";
const SPLITTING_KINDS: &[&str] = &[SIMPLE_EXPANSION, EXPANSION, "command_substitution"];
const BRACE_EXPRESSION: &str = "brace_expression";
const QUOTED_KINDS: &[&str] = &[STRING, RAW_STRING, ANSI_STRING, "translated_string"];

// Las partes sin comillas de una palabra pegadas, con lo que está entre comillas como NUL.
fn unquoted_text(word: Node<'_>, source: &[u8]) -> Vec<u8> {
    let pieces = if word.kind() == CONCATENATION {
        children(word)
    } else {
        vec![word]
    };
    let mut joined = Vec::new();
    for piece in pieces {
        let bytes = source
            .get(piece.start_byte()..piece.end_byte())
            .unwrap_or_default();
        if piece.kind() == WORD || piece.kind() == NUMBER || !piece.is_named() {
            joined.extend_from_slice(bytes);
        } else {
            joined.extend(std::iter::repeat_n(0, bytes.len()));
        }
    }
    joined
}

// `isGlob` de ShellCheck: un `*`, `?`, una clase `[...]` o un extglob sin comillas.
pub fn is_glob(word: Node<'_>, source: &[u8]) -> bool {
    if QUOTED_KINDS.contains(&word.kind()) {
        return false;
    }
    let unquoted = unquoted_text(word, source);
    let mut index = 0;
    let mut open_class = false;
    while let Some(byte) = unquoted.get(index) {
        if *byte == ESCAPE {
            index = index.saturating_add(2);
            continue;
        }
        if GLOB_CHARACTERS.contains(byte) {
            return true;
        }
        if *byte == OPEN_CLASS {
            open_class = true;
        } else if *byte == CLOSE_CLASS && open_class {
            return true;
        }
        index = index.saturating_add(1);
    }
    let raw = String::from_utf8_lossy(&unquoted);
    EXTGLOB_STARTS.iter().any(|start| raw.contains(start))
}

// `willSplit`: una expansión, un glob o llaves sin comillas, o algo entre comillas que se vuelve
// varios argumentos (`"$@"`).
pub fn will_split(word: Node<'_>, source: &[u8]) -> bool {
    match word.kind() {
        STRING => named_children(word)
            .into_iter()
            .any(|part| is_array_part(part, source)),
        CONCATENATION => {
            is_brace_expansion(word, source)
                || children(word).into_iter().any(|part| will_split(part, source))
        }
        BRACE_EXPRESSION => true,
        kind if SPLITTING_KINDS.contains(&kind) => true,
        _ => is_glob(word, source),
    }
}

fn is_array_part(part: Node<'_>, source: &[u8]) -> bool {
    (part.kind() == SIMPLE_EXPANSION || part.kind() == EXPANSION)
        && is_array_expansion(&braced_content(part, source))
}

// `mayBecomeMultipleArgs`: un glob, llaves, un arreglo o una expansión sin comillas, o `"${!x}"`.
pub fn may_become_multiple_args(word: Node<'_>, source: &[u8]) -> bool {
    if is_glob(word, source)
        || word.kind() == BRACE_EXPRESSION
        || (word.kind() == CONCATENATION && is_brace_expansion(word, source))
    {
        return true;
    }
    let pieces = if word.kind() == CONCATENATION {
        children(word)
    } else {
        vec![word]
    };
    pieces.into_iter().any(|piece| match piece.kind() {
        SIMPLE_EXPANSION | EXPANSION => true,
        STRING => named_children(piece).into_iter().any(|part| {
            is_array_part(part, source)
                || (part.kind() == EXPANSION && braced_content(part, source).starts_with(INDIRECTION))
        }),
        _ => false,
    })
}

// `getSingleUnmodifiedBracedString`: la palabra es sólo `$x` o `${x}`, con o sin comillas.
pub fn single_unmodified_expansion(word: Node<'_>, source: &[u8]) -> Option<String> {
    let [part] = parts(word)[..] else {
        return None;
    };
    if part.kind() != SIMPLE_EXPANSION && part.kind() != EXPANSION {
        return None;
    }
    let contents = braced_content(part, source);
    (reference(&contents) == contents).then_some(contents)
}

// Las partes de una palabra como las da `getWordParts`: lo de dentro de las comillas dobles suelto.
pub fn word_parts(word: Node<'_>) -> Vec<Node<'_>> {
    parts(word)
}

pub fn is_literal_part(part: Node<'_>, source: &[u8]) -> bool {
    part.kind() == STRING_CONTENT || literal(part, source).is_some() || text(part, source).is_empty()
}

// `getLiteralStringDef`: el texto literal con cada parte que no lo es (un glob, una expansión)
// cambiada por el reemplazo.
pub fn literal_with_globs(word: Node<'_>, source: &[u8], fallback: &str) -> String {
    let pieces = if word.kind() == CONCATENATION {
        children(word)
    } else {
        vec![word]
    };
    pieces
        .into_iter()
        .map(|piece| literal(piece, source).unwrap_or_else(|| piece_with_globs(piece, source, fallback)))
        .collect()
}

fn piece_with_globs(piece: Node<'_>, source: &[u8], fallback: &str) -> String {
    if piece.kind() != WORD {
        return literal_or(piece, source, fallback);
    }
    let is_glob_character =
        |character: char| GLOB_CHARACTERS.contains(&u8::try_from(character).unwrap_or_default());
    text(piece, source)
        .chars()
        .map(|character| {
            if is_glob_character(character) {
                fallback.to_owned()
            } else {
                character.to_string()
            }
        })
        .collect()
}
