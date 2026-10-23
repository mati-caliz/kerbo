use std::collections::HashSet;

use tree_sitter::Node;

use super::java::text;

const PARENTHESIZED: &str = "parenthesized_expression";
const UNARY: &str = "unary_expression";
const OPERATOR_FIELD: &str = "operator";
const NOT: &str = "!";

// Qué hace quien llama con el resultado: lo tira, lo usa, o lo usa negado (`!activo()`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultUse {
    Discarded,
    Used,
    Negated,
}

// Un `!` delante, aunque haya paréntesis en el medio, es usarlo negado.
pub fn result_use(node: Node<'_>, discarded: &HashSet<usize>, source: &[u8]) -> ResultUse {
    if discarded.contains(&node.id()) {
        return ResultUse::Discarded;
    }
    let mut parent = node.parent();
    while let Some(wrapper) = parent.filter(|wrapper| wrapper.kind() == PARENTHESIZED) {
        parent = wrapper.parent();
    }
    let negated = parent.is_some_and(|unary| {
        unary.kind() == UNARY
            && unary
                .child_by_field_name(OPERATOR_FIELD)
                .is_some_and(|operator| text(operator, source) == NOT)
    });
    if negated {
        ResultUse::Negated
    } else {
        ResultUse::Used
    }
}
