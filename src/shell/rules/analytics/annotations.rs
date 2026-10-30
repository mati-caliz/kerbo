use tree_sitter::Node;

use crate::shell::directives::disables;
use crate::shell::syntax::{COMMENT, PROGRAM, children, text};

const COMMENT_START: char = '#';
const DIRECTIVE_PREFIX: &str = "shellcheck";
const KEY_VALUE_SEPARATOR: char = '=';
const LIST_SEPARATOR: char = ',';
const QUOTES: &[char] = &['\'', '"'];
const SOURCE_KEY: &str = "source";
const DISABLE_KEY: &str = "disable";

// Las claves de las directivas `# shellcheck` que valen para el nodo, de la más cercana a la más
// lejana: las de los comentarios justo antes del nodo o de algo que lo contiene, y las del
// principio del archivo.
fn annotations(node: Node<'_>, source: &[u8]) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let mut current = Some(node);
    while let Some(statement) = current {
        let mut previous = statement.prev_sibling();
        while let Some(comment) = previous.filter(|sibling| sibling.kind() == COMMENT) {
            found.extend(directive_pairs(text(comment, source)));
            previous = comment.prev_sibling();
        }
        if statement.kind() == PROGRAM {
            found.extend(
                children(statement)
                    .into_iter()
                    .take_while(|child| child.kind() == COMMENT)
                    .flat_map(|comment| directive_pairs(text(comment, source))),
            );
            break;
        }
        current = statement.parent();
    }
    found
}

fn directive_pairs(comment: &str) -> Vec<(String, String)> {
    let Some(body) = comment.strip_prefix(COMMENT_START).map(str::trim_start) else {
        return Vec::new();
    };
    let Some(pairs) = body
        .strip_prefix(DIRECTIVE_PREFIX)
        .filter(|pairs| pairs.starts_with(char::is_whitespace))
    else {
        return Vec::new();
    };
    pairs
        .split_whitespace()
        .take_while(|pair| !pair.starts_with(COMMENT_START))
        .map_while(|pair| pair.split_once(KEY_VALUE_SEPARATOR))
        .map(|(key, value)| (key.to_owned(), value.trim_matches(QUOTES).to_owned()))
        .collect()
}

// `# shellcheck source=archivo`: el archivo que ShellCheck lee en lugar del que nombra el `source`.
pub fn source_override(node: Node<'_>, source: &[u8]) -> Option<String> {
    annotations(node, source)
        .into_iter()
        .find(|(key, _)| key == SOURCE_KEY)
        .map(|(_, value)| value)
}

// `shouldIgnoreCode`: una directiva `disable=` que alcanza al nodo nombra la regla.
pub fn is_disabled(node: Node<'_>, source: &[u8], rule: &str) -> bool {
    annotations(node, source)
        .iter()
        .filter(|(key, _)| key == DISABLE_KEY)
        .flat_map(|(_, value)| value.split(LIST_SEPARATOR))
        .any(|element| disables(element, rule))
}
