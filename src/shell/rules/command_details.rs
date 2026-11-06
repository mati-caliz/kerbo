mod array_loop_index;
mod brace_ranges;
mod declarations;
mod getopts;
mod printf_formats;
mod spacing_and_bangs;

use tree_sitter::Node;

use crate::shell::context::{Context, Findings};

// Chequeos de `Checks/Commands.hs` y `Checks/ShellSupport.hs` de ShellCheck que miran el detalle de un
// comando: el formato de `printf`, el `case` de `getopts`, las declaraciones y lo que cada shell soporta.
pub fn check(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    printf_formats::check(node, context.source, findings);
    getopts::check(node, context, findings);
    declarations::check(node, context.source, findings);
    brace_ranges::check(node, context, findings);
    spacing_and_bangs::check(node, context, findings);
    array_loop_index::check(node, context.source, findings);
}
