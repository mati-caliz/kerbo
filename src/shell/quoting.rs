use tree_sitter::Node;

use super::commands::Command;
use super::context::Context;
use super::syntax::{
    ARITHMETIC_EXPANSION, ARITHMETIC_FOR, ASSIGNMENT, CASE, CASE_ITEM, COMMAND, COMMAND_NAME,
    CONDITION_FIELD, DECLARATION, ELIF_CLAUSE, EXPANSION, FILE_REDIRECT, FOR, HEREDOC_BODY, HEREDOC_REDIRECT,
    HERESTRING, IF, LIST, REDIRECTED, STRING, TEST, TRANSLATED_STRING, UNSET, WHILE, children,
    field_children, is_arithmetic_command, is_double_bracket, named_children,
};

const THEN: &str = "then";
const STOPPING_CONTEXTS: &[&str] = &[
    COMMAND,
    DECLARATION,
    UNSET,
    REDIRECTED,
    FILE_REDIRECT,
    HERESTRING,
    HEREDOC_REDIRECT,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strictness {
    Strict,
    Lenient,
}

pub fn is_quote_free(node: Node<'_>, context: &Context<'_, '_>, strictness: Strictness) -> bool {
    let mut child = node;
    while let Some(parent) = child.parent() {
        if let Some(decided) = quote_free_context(parent, child, context, strictness) {
            return decided;
        }
        child = parent;
    }
    false
}

fn quote_free_context(
    parent: Node<'_>,
    child: Node<'_>,
    context: &Context<'_, '_>,
    strictness: Strictness,
) -> Option<bool> {
    let source = context.source;
    match parent.kind() {
        STRING | TRANSLATED_STRING | EXPANSION | ARITHMETIC_EXPANSION | ARITHMETIC_FOR | CASE | CASE_ITEM
        | HEREDOC_BODY => Some(true),
        ASSIGNMENT => Some(
            context.dialect.parses_declaration_assignments()
                || parent.parent().is_none_or(|holder| holder.kind() != DECLARATION),
        ),
        TEST => Some(is_double_bracket(parent, source)),
        FOR if is_loop_value(parent, child) => Some(strictness == Strictness::Lenient),
        kind if is_arithmetic_command(parent, source) || STOPPING_CONTEXTS.contains(&kind) => {
            Some(is_arithmetic_command(parent, source))
        }
        _ => None,
    }
}

fn is_loop_value(loop_node: Node<'_>, child: Node<'_>) -> bool {
    field_children(loop_node, super::syntax::VALUE_FIELD).contains(&child)
}

pub fn used_as_command_name(node: Node<'_>, source: &[u8]) -> bool {
    let mut current = node;
    while let Some(parent) = current.parent() {
        let single_part = named_children(parent).len() == 1;
        if (parent.kind() == STRING || parent.kind() == super::syntax::CONCATENATION) && single_part {
            current = parent;
            continue;
        }
        if parent.kind() == COMMAND_NAME {
            return true;
        }
        return Command::of(parent).is_some_and(|command| command.name_word(source) == Some(current));
    }
    false
}

pub fn is_condition(node: Node<'_>, source: &[u8]) -> bool {
    let mut child = node;
    while let Some(parent) = child.parent() {
        if condition_children(parent, source).contains(&child) {
            return true;
        }
        child = parent;
    }
    false
}

fn condition_children<'tree>(parent: Node<'tree>, source: &[u8]) -> Vec<Node<'tree>> {
    match parent.kind() {
        LIST => named_children(parent).into_iter().take(1).collect(),
        IF | WHILE => field_children(parent, CONDITION_FIELD)
            .into_iter()
            .rfind(Node::is_named)
            .into_iter()
            .collect(),
        ELIF_CLAUSE => children(parent)
            .into_iter()
            .take_while(|child| super::syntax::text(*child, source) != THEN)
            .filter(Node::is_named)
            .filter(|child| child.kind() != super::syntax::COMMENT)
            .last()
            .into_iter()
            .collect(),
        _ => Vec::new(),
    }
}
