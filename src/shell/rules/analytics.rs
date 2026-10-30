mod annotations;
mod functions;
mod pipelines;
mod quotes;
mod sources;
mod words;

use tree_sitter::Node;

use crate::shell::context::{Context, Findings};

// Los chequeos de `Analytics.hs` de ShellCheck que no piden el flujo de datos, más los `source` que
// el parser no sigue (SC1090, SC1091).
pub fn check(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    sources::check(node, context, findings);
    pipelines::check(node, context, findings);
    words::check(node, context, findings);
    quotes::check(node, context, findings);
}

pub fn check_script(context: &Context<'_, '_>, findings: &mut Findings) {
    functions::check(context, findings);
}
