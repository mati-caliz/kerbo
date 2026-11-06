mod exec_cleanup;
mod external_functions;
mod find_exec;
mod for_loops;
mod globs_and_prefixes;
mod loop_keywords;
mod misc;
mod read_loops;
mod useless_bang;

use tree_sitter::Node;

use crate::shell::context::{Context, Findings};

// Chequeos de `Analytics.hs` de ShellCheck sobre el flujo del script: loops, `exec`, `break`, funciones
// que se pasan a otro comando, `ssh` adentro de un `while read` y lo que queda en el shebang.
pub fn check(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    for_loops::check(node, context.source, findings);
    find_exec::check(node, context.source, findings);
    exec_cleanup::check(node, context, findings);
    loop_keywords::check(node, context, findings);
    read_loops::check(node, context.source, findings);
    globs_and_prefixes::check(node, context.source, findings);
    misc::check(node, context, findings);
    useless_bang::check(node, context, findings);
}

pub fn check_script(context: &Context<'_, '_>, findings: &mut Findings) {
    external_functions::check(context, findings);
    misc::check_shebang(context, findings);
}
