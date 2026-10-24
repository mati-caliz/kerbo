use tree_sitter::Node;

use super::file_loops::loop_writes;
use super::if_blocks::if_reports;
use super::model::Model;
use super::stub_assignments::stub_assignment_reports;
use super::stub_conditions::stub_condition_reports;
use super::timeouts::timeout_without_checkpoint;
use crate::lint::python::Report;
use crate::lint::python::ast::EXPRESSION_STATEMENT;

const IF: &str = "if_statement";
const FOR: &str = "for_statement";
const WITH: &str = "with_statement";

// Chequeos de sentencias que necesitan resolver nombres, antes de recorrerlas: UP036, PYI006 y SIM401
// en un `if`, FURB122 en un `for`, ASYNC100 en un `with`, y en un stub las condiciones y las
// asignaciones que sólo se miran ahí.
pub fn compound_reports(model: &Model<'_>, source: &[u8], statement: Node<'_>, stub: bool) -> Vec<Report> {
    let mut reports = match statement.kind() {
        IF => if_reports(model, source, statement),
        FOR => loop_writes(model, source, statement).into_iter().collect(),
        WITH => timeout_without_checkpoint(model, source, statement)
            .into_iter()
            .collect(),
        _ => Vec::new(),
    };
    if stub {
        match statement.kind() {
            IF => reports.extend(stub_condition_reports(model, source, statement)),
            EXPRESSION_STATEMENT => reports.extend(stub_assignment_reports(model, source, statement)),
            _ => {}
        }
    }
    reports
}
