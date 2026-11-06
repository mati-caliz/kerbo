mod aliases;
mod arrays;
mod equals_in_name;
mod names;
mod pipes;
mod redirect_targets;

use tree_sitter::Node;

use crate::shell::context::{Context, Findings};

// Chequeos de `Analytics.hs` de ShellCheck sobre la forma de un comando: nombres que no pueden ser un
// comando, pipes y redirecciones que no llevan a nada, y elementos de arreglos mal escritos.
pub fn check(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    names::check(node, context, findings);
    equals_in_name::check(node, context, findings);
    pipes::check(node, context.source, findings);
    redirect_targets::check(node, context.source, findings);
    arrays::check(node, context, findings);
}

pub fn check_script(context: &Context<'_, '_>, findings: &mut Findings) {
    aliases::check(context, findings);
}
