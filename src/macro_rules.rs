mod definition;
mod expansion;
mod expressions;
mod fragments;
mod matcher;
mod reader;
mod registry;
mod scope;
mod tokens;
mod transcriber;

use tree_sitter::Node;

pub use expansion::{Code, Expansion};
use registry::Context;
pub use registry::{CrateMacros, MacroIndex};
pub use scope::MacroScope;

const PATH_SEPARATOR: &str = "::";
const RELATIVE_ROOTS: &[&str] = &["crate", "$crate", "self", "super"];
const STATEMENT: &str = "expression_statement";
const SOURCE_FILE: &str = "source_file";
const DECLARATIONS: &str = "declaration_list";
const IMPLEMENTATION: &str = "impl_item";
const TRAIT: &str = "trait_item";
const TRAIT_FIELD: &str = "trait";

// Sólo las macros del propio crate: una ruta que nombra otro crate (`log::debug!`) es de una
// dependencia, y `crate::`, `self::` o `super::` siguen dentro del crate.
fn local_name(path: &str) -> Option<&str> {
    let segments: Vec<&str> = path.split(PATH_SEPARATOR).map(str::trim).collect();
    match segments.as_slice() {
        [name] => Some(name),
        [root, .., name] if RELATIVE_ROOTS.contains(root) => Some(name),
        _ => None,
    }
}

fn context_of(invocation: Node<'_>) -> Context {
    let parent = invocation
        .parent()
        .filter(|parent| parent.kind() == STATEMENT)
        .and_then(|statement| statement.parent())
        .or_else(|| invocation.parent());
    let Some(parent) = parent else {
        return Context::Items;
    };
    match parent.kind() {
        SOURCE_FILE => Context::Items,
        DECLARATIONS => match parent.parent() {
            Some(container) if container.kind() == IMPLEMENTATION => Context::ImplItems {
                implements_trait: container.child_by_field_name(TRAIT_FIELD).is_some(),
            },
            Some(container) if container.kind() == TRAIT => Context::TraitItems,
            _ => Context::Items,
        },
        _ => Context::Statements,
    }
}

#[cfg(test)]
mod tests;
