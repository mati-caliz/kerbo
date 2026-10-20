mod eradicate;
mod python_tokens;

use tree_sitter::Node;

use crate::profile::CommentedCodeStyle;
use crate::syntax::one_based;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentedCode {
    pub line: usize,
    pub column: usize,
    pub text: String,
}

pub fn find_commented_code(root: Node<'_>, source: &[u8], style: CommentedCodeStyle) -> Vec<CommentedCode> {
    match style {
        CommentedCodeStyle::Eradicate => eradicate::reported(root, source),
        CommentedCodeStyle::Unmeasured => Vec::new(),
    }
}

fn at(node: Node<'_>, source: &[u8]) -> CommentedCode {
    let start = node.start_position();
    CommentedCode {
        line: one_based(start.row),
        column: one_based(start.column),
        text: node.utf8_text(source).unwrap_or_default().trim().to_owned(),
    }
}

#[cfg(test)]
mod tests;
