use tree_sitter::Node;

use super::{ShortName, field, has_token, named_children, short_name, text};

const MIN_LENGTH: usize = 3;
const EXCEPTIONS: &[&str] = &["i", "j"];
const LOCAL_VARIABLE: &str = "local_variable_declaration";
const STATEMENT_LISTS: &[&str] = &["block", "constructor_body", "switch_block_statement_group"];
const FOR: &str = "for_statement";
const FOR_EACH: &str = "enhanced_for_statement";
const DECLARATOR: &str = "variable_declarator";
const MODIFIERS: &str = "modifiers";
const FINAL: &str = "final";

pub fn reported(nodes: &[Node<'_>], source: &[u8]) -> Vec<ShortName> {
    nodes
        .iter()
        .filter(|node| !is_final(**node))
        .flat_map(|&node| local_names(node))
        .filter_map(|name| {
            let name_text = text(name, source);
            (name_text.chars().count() < MIN_LENGTH && !EXCEPTIONS.contains(&name_text))
                .then(|| short_name(name, name_text))
        })
        .collect()
}

fn local_names(node: Node<'_>) -> Vec<Node<'_>> {
    match node.kind() {
        LOCAL_VARIABLE if node.parent().is_some_and(is_local_scope) => named_children(node)
            .into_iter()
            .filter(|child| child.kind() == DECLARATOR)
            .filter_map(|declarator| field(declarator, "name"))
            .collect(),
        FOR_EACH => field(node, "name").into_iter().collect(),
        _ => Vec::new(),
    }
}

fn is_local_scope(parent: Node<'_>) -> bool {
    STATEMENT_LISTS.contains(&parent.kind()) || parent.kind() == FOR
}

fn is_final(node: Node<'_>) -> bool {
    named_children(node)
        .into_iter()
        .any(|child| child.kind() == MODIFIERS && has_token(child, FINAL))
}
