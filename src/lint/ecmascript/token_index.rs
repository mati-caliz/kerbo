use tree_sitter::Node;

use super::nodes::COMMENT;

#[derive(Debug, Clone, Copy)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

// Los tokens del archivo en orden, sin comentarios, para `getTokenBefore` y `getCommentsBefore`.
pub struct TokenIndex<'tree> {
    tokens: Vec<Span>,
    comments: Vec<Node<'tree>>,
}

impl<'tree> TokenIndex<'tree> {
    pub fn new(root: Node<'tree>) -> Self {
        let mut tokens = Vec::new();
        let mut comments = Vec::new();
        let mut pending = vec![root];
        while let Some(node) = pending.pop() {
            if node.kind() == COMMENT {
                comments.push(node);
                continue;
            }
            if node.child_count() == 0 {
                tokens.extend(span_of(node));
                continue;
            }
            let mut cursor = node.walk();
            let first_pending = pending.len();
            pending.extend(node.children(&mut cursor));
            pending.get_mut(first_pending..).unwrap_or_default().reverse();
        }
        comments.sort_by_key(Node::start_byte);
        Self { tokens, comments }
    }

    pub fn token_before(&self, offset: usize) -> Option<Span> {
        let index = self.tokens.partition_point(|token| token.end <= offset);
        index
            .checked_sub(1)
            .and_then(|previous| self.tokens.get(previous))
            .copied()
    }

    // Los comentarios entre el token anterior y la posición, en orden.
    pub fn comments_before(&self, offset: usize) -> Vec<Node<'tree>> {
        let from = self.token_before(offset).map_or(0, |token| token.end);
        self.comments
            .iter()
            .copied()
            .filter(|comment| comment.start_byte() >= from && comment.end_byte() <= offset)
            .collect()
    }
}

fn span_of(leaf: Node<'_>) -> Option<Span> {
    (leaf.end_byte() > leaf.start_byte()).then(|| Span {
        start: leaf.start_byte(),
        end: leaf.end_byte(),
    })
}
