mod analyzer;
mod contexts;
mod flow;
mod loops;
mod reachability;
mod roles;
mod segments;
mod shapes;
mod state;

pub use reachability::Reachability;
pub use shapes::case_statements;

use tree_sitter::Node;

pub fn reachability(root: Node<'_>, source: &[u8]) -> Reachability {
    analyzer::Analyzer::analyze(root, source)
}
