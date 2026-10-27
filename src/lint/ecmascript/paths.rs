mod analyzer;
mod contexts;
mod flow;
mod loops;
mod reachability;
mod roles;
mod segments;
mod shapes;
mod state;
mod supers;
mod traversal;

pub use reachability::{Flow, PathRecord};
pub use shapes::case_statements;
pub use supers::{DerivedConstructor, SuperProblem};

use tree_sitter::Node;

pub fn flow<'tree>(root: Node<'tree>, source: &[u8]) -> Flow<'tree> {
    analyzer::Analyzer::analyze(root, source)
}
