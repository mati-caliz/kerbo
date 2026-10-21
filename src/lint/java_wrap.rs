use super::java_tokens::Token;
use super::{LintFinding, LintKind, finding};
use crate::rules::JavaRule;

const BINARY: &str = "binary_expression";
const TERNARY: &str = "ternary_expression";
const QUESTION: &str = "?";
const COLON: &str = ":";
const COLON_OWNERS: &[&str] = &[TERNARY, "enhanced_for_statement", "assert_statement"];
const TYPE_AND: &str = "&";
const TYPE_AND_OWNERS: &[&str] = &["type_bound", "cast_expression"];
const CATCH_OR: &str = "|";
const CATCH_TYPE: &str = "catch_type";
const INSTANCEOF: &str = "instanceof";
const PARENTHESIZED: &str = "parenthesized_expression";
const OPEN_PAREN: &str = "(";
const CLOSE_PAREN: &str = ")";

// Calca OperatorWrap con nl: un operador binario, los de un ternario, el `:` de un for mejorado o un
// assert, el `&` de los límites de un genérico, el `|` de un multi-catch e `instanceof` no terminan
// la línea: van al principio de la siguiente. Los paréntesis que envuelven un operando no cuentan.
pub fn reported(tokens: &[Token<'_>]) -> Vec<LintFinding> {
    let code: Vec<Token<'_>> = tokens
        .iter()
        .copied()
        .filter(|token| !token.is_comment())
        .collect();
    code.iter()
        .enumerate()
        .filter(|(_, token)| wraps(token))
        .filter_map(|(index, token)| {
            let left = code.get(..index)?.iter().rev().find(|previous| {
                !(previous.kind() == CLOSE_PAREN && previous.parent_kind() == PARENTHESIZED)
            })?;
            let right = code
                .get(index.saturating_add(1)..)?
                .iter()
                .find(|next| !(next.kind() == OPEN_PAREN && next.parent_kind() == PARENTHESIZED))?;
            let row = token.start().row;
            (left.start().row == row && right.start().row != row).then(|| {
                finding(
                    LintKind::Java(JavaRule::OperatorWrap),
                    token.node,
                    token.kind(),
                    None,
                )
            })
        })
        .collect()
}

fn wraps(token: &Token<'_>) -> bool {
    let parent = token.parent_kind();
    match token.kind() {
        QUESTION => parent == TERNARY,
        COLON => COLON_OWNERS.contains(&parent),
        INSTANCEOF => true,
        TYPE_AND if TYPE_AND_OWNERS.contains(&parent) => true,
        CATCH_OR if parent == CATCH_TYPE => true,
        _ => parent == BINARY && !token.node.is_named(),
    }
}
