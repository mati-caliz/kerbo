use tree_sitter::Node;

use crate::lint::java::text;
use crate::lint::taint::Literal;
use crate::syntax::named_children;

const STRING_FRAGMENT: &str = "string_fragment";
const QUOTE: char = '\'';
const LONG_SUFFIXES: &[char] = &['L', 'l'];
const DIGIT_SEPARATOR: char = '_';
const DECIMAL: &str = "decimal_integer_literal";
const CHARACTER: &str = "character_literal";
const STRING: &str = "string_literal";
const TRUE: &str = "true";
const FALSE: &str = "false";

// El valor de un literal que se puede seguir sin dudas: un texto sin escapes, un entero decimal,
// un carácter simple o un booleano.
pub fn literal_of(node: Node<'_>, source: &[u8]) -> Option<Literal> {
    match node.kind() {
        STRING => {
            let parts = named_children(node);
            parts
                .iter()
                .all(|part| part.kind() == STRING_FRAGMENT)
                .then(|| Literal::Text(parts.iter().map(|part| text(*part, source)).collect()))
        }
        DECIMAL => text(node, source)
            .trim_end_matches(LONG_SUFFIXES)
            .replace(DIGIT_SEPARATOR, "")
            .parse()
            .ok()
            .map(Literal::Integer),
        CHARACTER => {
            let inner = text(node, source).strip_prefix(QUOTE)?.strip_suffix(QUOTE)?;
            let mut characters = inner.chars();
            match (characters.next(), characters.next()) {
                (Some(single), None) => Some(Literal::Character(single)),
                _ => None,
            }
        }
        TRUE => Some(Literal::Boolean(true)),
        FALSE => Some(Literal::Boolean(false)),
        _ => None,
    }
}
