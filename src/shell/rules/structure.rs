mod assignments;
pub mod cases;
mod loops;
mod redirections;
mod sequences;
mod subshell_tests;
mod test_operands;

use tree_sitter::Node;

use crate::shell::context::{Context, Findings};

// Más chequeos de `Analytics.hs` de ShellCheck que no piden el flujo de datos: los operandos de las
// condiciones, los `case`, los subshells, las redirecciones y las asignaciones.
pub fn check(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    test_operands::check(node, context, findings);
    cases::check(node, context, findings);
    subshell_tests::check(node, context, findings);
    sequences::check(node, context, findings);
    redirections::check(node, context, findings);
    assignments::check(node, context, findings);
    loops::check(node, context, findings);
}
