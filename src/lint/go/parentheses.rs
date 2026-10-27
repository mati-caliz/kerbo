use tree_sitter::Node;

use crate::syntax::named_children;

const PARENTHESIZED: &str = "parenthesized_expression";
const COMMENT: &str = "comment";

pub fn without_parentheses(mut expression: Node<'_>) -> Node<'_> {
    while expression.kind() == PARENTHESIZED {
        let Some(inner) = named_children(expression)
            .into_iter()
            .find(|child| child.kind() != COMMENT)
        else {
            break;
        };
        expression = inner;
    }
    expression
}
