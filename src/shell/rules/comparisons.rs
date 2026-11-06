mod arithmetic;
mod assignments;
mod command_lists;
mod numbers;
mod numeric_values;
mod simple_commands;
mod test_operators;

use tree_sitter::Node;

pub use numeric_values::NumericValues;

use crate::shell::context::{Context, Findings};

// Los chequeos de `Analytics.hs` de ShellCheck sobre lo que se compara y cómo: operadores de `[ ]` y
// `[[ ]]`, condiciones encadenadas con `&&` y `||`, aritmética y comandos que parecen una comparación.
pub fn check(
    node: Node<'_>,
    context: &Context<'_, '_>,
    numeric_values: &NumericValues,
    findings: &mut Findings,
) {
    test_operators::check(node, context, numeric_values, findings);
    command_lists::check(node, context, findings);
    simple_commands::check(node, context.source, findings);
    assignments::check(node, context, numeric_values, findings);
    arithmetic::check(node, context, findings);
}
