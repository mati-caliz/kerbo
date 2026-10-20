use tree_sitter::Node;

use super::has_modifier;

const BODY_FIELD: &str = "body";
const ASYNC: &str = "async";
const BLOCK_START: char = '{';
const BLOCK_END: char = '}';
const COMMENT_START: &str = "/*";
const COMMENT_END: &str = "*/";
const LINE_COMMENT: &str = "//";
const MARKER_LENGTH: usize = 2;

// Calca too_many_lines de clippy, que lee el texto y no el árbol: cuenta las líneas del cuerpo con
// algo fuera de un comentario, así que un "//" o un "/*" adentro de un string cuenta como
// comentario igual que allá. El cuerpo de una función async es el bloque async que arma rustc, que
// clippy no reconoce como bloque: ahí las llaves quedan y suman sus líneas.
pub(super) fn too_many_lines(function: Node<'_>, source: &[u8]) -> Option<usize> {
    let text = function.child_by_field_name(BODY_FIELD)?.utf8_text(source).ok()?;
    let inner = if has_modifier(function, ASYNC) {
        text
    } else {
        text.strip_prefix(BLOCK_START)
            .and_then(|rest| rest.strip_suffix(BLOCK_END))
            .unwrap_or(text)
    };
    let mut in_comment = false;
    Some(
        inner
            .trim()
            .lines()
            .filter(|&line| has_code(line, &mut in_comment))
            .count(),
    )
}

fn has_code(line: &str, in_comment: &mut bool) -> bool {
    let mut rest = line;
    let mut code = false;
    loop {
        rest = rest.trim_start();
        if rest.is_empty() {
            break;
        }
        if *in_comment {
            if let Some(end) = rest.find(COMMENT_END) {
                rest = rest.get(end.saturating_add(MARKER_LENGTH)..).unwrap_or_default();
                *in_comment = false;
                continue;
            }
        } else {
            let block = rest.find(COMMENT_START).unwrap_or(rest.len());
            let single = rest.find(LINE_COMMENT).unwrap_or(rest.len());
            code |= block > 0 && single > 0;
            if block < single {
                rest = rest
                    .get(block.saturating_add(MARKER_LENGTH)..)
                    .unwrap_or_default();
                *in_comment = true;
                continue;
            }
        }
        break;
    }
    code
}
