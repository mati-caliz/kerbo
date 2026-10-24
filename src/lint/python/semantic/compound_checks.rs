use tree_sitter::Node;

use super::file_loops::loop_writes;
use super::if_blocks::if_reports;
use super::model::Model;
use super::timeouts::timeout_without_checkpoint;
use crate::lint::python::Report;

const IF: &str = "if_statement";
const FOR: &str = "for_statement";
const WITH: &str = "with_statement";

// Chequeos de sentencias compuestas que necesitan resolver nombres, antes de recorrerlas: UP036,
// PYI006 y SIM401 en un `if`, FURB122 en un `for` y ASYNC100 en un `with`.
pub fn compound_reports(model: &Model<'_>, source: &[u8], statement: Node<'_>) -> Vec<Report> {
    match statement.kind() {
        IF => if_reports(model, source, statement),
        FOR => loop_writes(model, source, statement).into_iter().collect(),
        WITH => timeout_without_checkpoint(model, source, statement)
            .into_iter()
            .collect(),
        _ => Vec::new(),
    }
}
