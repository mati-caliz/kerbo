mod document;
mod go;
mod python;

use tree_sitter::{Node, Tree};

use crate::syntax::descendants;

pub use go::format as format_go;
pub use python::format as format_python;

const FIRST_LINE: usize = 1;
const NO_TREE: Unparsable = Unparsable::SyntaxError { line: FIRST_LINE };

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unparsable {
    SyntaxError { line: usize },
    ReadsUnlikeGo { line: usize },
    Unsupported { line: usize },
    UnsupportedSyntax { line: usize, syntax: String },
}

fn line_of(node: Node<'_>) -> usize {
    node.start_position().row.saturating_add(FIRST_LINE)
}

fn syntax_error(tree: &Tree) -> Unparsable {
    let broken = descendants(tree.root_node())
        .into_iter()
        .find(|node| node.is_error() || node.is_missing());
    Unparsable::SyntaxError {
        line: broken.map_or(FIRST_LINE, line_of),
    }
}
