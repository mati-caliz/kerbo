mod definition;
mod dependencies;
mod expansion;
mod expressions;
mod foreign;
mod fragments;
mod imports;
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
const LOCAL_ROOTS: &[&str] = &["crate", "self", "super"];
const DOLLAR_CRATE: &str = "$crate";
const STATEMENT: &str = "expression_statement";
const SOURCE_FILE: &str = "source_file";
const DECLARATIONS: &str = "declaration_list";
const IMPLEMENTATION: &str = "impl_item";
const TRAIT: &str = "trait_item";
const TRAIT_FIELD: &str = "trait";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacroPath<'path> {
    Plain(&'path str),
    DollarCrate(&'path str),
    Foreign { krate: &'path str, name: &'path str },
}

// `crate::`, `self::` o `super::` siguen dentro del crate; `$crate::` es el crate de donde viene la
// definición que se está expandiendo, y una ruta que nombra otro crate (`log::debug!`) es de una
// dependencia.
fn macro_path(path: &str) -> Option<MacroPath<'_>> {
    let segments: Vec<&str> = path.split(PATH_SEPARATOR).map(str::trim).collect();
    match segments.as_slice() {
        [name] => Some(MacroPath::Plain(name)),
        [root, .., name] if *root == DOLLAR_CRATE => Some(MacroPath::DollarCrate(name)),
        [root, .., name] if LOCAL_ROOTS.contains(root) => Some(MacroPath::Plain(name)),
        ["", krate, .., name] | [krate, .., name] => Some(MacroPath::Foreign { krate, name }),
        [] => None,
    }
}

fn local_name(path: &str) -> Option<&str> {
    match macro_path(path)? {
        MacroPath::Plain(name) | MacroPath::DollarCrate(name) => Some(name),
        MacroPath::Foreign { .. } => None,
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
