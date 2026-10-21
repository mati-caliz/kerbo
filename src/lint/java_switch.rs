use tree_sitter::Node;

use crate::syntax::named_children;

const EXPRESSION_STATEMENT: &str = "expression_statement";
const BODY_FIELD: &str = "body";
const SWITCH_RULE: &str = "switch_rule";
const STATEMENT_PARENTS: &[&str] = &[
    "block",
    "constructor_body",
    "switch_block_statement_group",
    "labeled_statement",
    "switch_rule",
    "if_statement",
    "while_statement",
    "for_statement",
    "enhanced_for_statement",
    "do_statement",
    EXPRESSION_STATEMENT,
];

// Un switch que es una expresión (asignado, devuelto, pasado) usa lo que da cada `case X -> ...`;
// uno escrito como sentencia lo tira.
pub fn yields_value(switch: Node<'_>) -> bool {
    switch
        .parent()
        .is_some_and(|parent| !STATEMENT_PARENTS.contains(&parent.kind()))
}

pub fn rule_bodies(switch: Node<'_>) -> Vec<usize> {
    switch
        .child_by_field_name(BODY_FIELD)
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .filter(|rule| rule.kind() == SWITCH_RULE)
        .flat_map(named_children)
        .filter(|body| body.kind() == EXPRESSION_STATEMENT)
        .map(|body| body.id())
        .collect()
}
