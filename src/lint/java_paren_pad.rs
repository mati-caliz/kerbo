use super::java_tokens::{Lines, Token};
use super::{LintFinding, LintKind, finding};
use crate::rules::JavaRule;

const OPEN_PAREN: &str = "(";
const CLOSE_PAREN: &str = ")";
const SEMICOLON: &str = ";";
const CAST: &str = "cast_expression";
const FOR: &str = "for_statement";
const RESOURCES: &str = "resource_specification";

// `ParenPad` de Checkstyle con `nospace`: un blanco después de `(` o antes de `)` en la misma línea.
// No se miran los paréntesis de un cast, el `(` de un for sin inicialización, el `)` de un for sin
// actualización ni el `)` de un try con recursos que sigue a un `;`. Un `(` al final de la línea o
// un `)` al principio tampoco.
pub fn reported(tokens: &[Token<'_>], lines: &Lines<'_>) -> Vec<LintFinding> {
    let code: Vec<Token<'_>> = tokens
        .iter()
        .copied()
        .filter(|token| !token.is_comment())
        .collect();
    code.iter()
        .enumerate()
        .filter(|(_, token)| token.parent_kind() != CAST)
        .filter_map(|(index, token)| {
            let next = code.get(index.saturating_add(1));
            let previous = index.checked_sub(1).and_then(|before| code.get(before));
            let padded = match token.kind() {
                OPEN_PAREN => {
                    let empty_init =
                        token.parent_kind() == FOR && next.is_some_and(|next| next.kind() == SEMICOLON);
                    let end = token.end();
                    !empty_init && end.column < lines.width(end.row) && lines.whitespace_at(end)
                }
                CLOSE_PAREN => {
                    let after_semicolon = (token.parent_kind() == FOR || token.parent_kind() == RESOURCES)
                        && previous.is_some_and(|previous| previous.kind() == SEMICOLON);
                    let start = token.start();
                    !after_semicolon && lines.whitespace_before(start) && !lines.is_blank_before(start)
                }
                _ => false,
            };
            padded.then(|| finding(LintKind::Java(JavaRule::ParenPad), token.node, token.kind(), None))
        })
        .collect()
}
