use std::sync::LazyLock;

use regex::bytes::Regex;
use tree_sitter::{Node, Tree};

use super::nodes::PARENTHESIZED;
use super::scope::bindings::has_token;
use crate::syntax::{SnippetLanguage, descendants, named_children, parse_snippet};

const AMPERSAND: u8 = b'&';
const EQUALS: u8 = b'=';
const BLANK: u8 = b' ';
const TYPE_ONLY_KIND: &str = "type_annotation";
const JSX_KIND: &str = "jsx_element";
const MODIFIERS_GROUP: usize = 2;
const TYPE_ARGUMENTS: &str = "type_arguments";
const TYPEOF: &str = "typeof";
const COLON: &str = ":";
const EXACT_OBJECT_OPENING: &str = "{|";
const MODULE: &str = "module";
const PROPERTY: &str = "property_identifier";
const EXPRESSIONS_WITH_TYPE_ARGUMENTS: &[&str] = &[
    "call_expression",
    "new_expression",
    "jsx_opening_element",
    "jsx_self_closing_element",
];

static CHARACTER_REFERENCE: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"\A&(?:#(?:[xX][0-9a-fA-F]{1,6}|[0-9]{1,5})|[A-Za-z]{1,30});").ok());
static VARIANCE: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(r"((?:[<,]|\*/)[ \t\r\n]*)((?:in[ \t\r\n]+)?out|in)[ \t\r\n]+[A-Za-z_$]").ok()
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Grammar {
    JavaScript,
    TypeScript,
    Tsx,
}

pub fn reparsed(root: Node<'_>, source: &[u8]) -> Option<Tree> {
    let grammar = grammar_of(root);
    let fallback = match grammar {
        Grammar::TypeScript => SnippetLanguage::TypeScript,
        Grammar::JavaScript | Grammar::Tsx => SnippetLanguage::Tsx,
    };
    let attempts = match grammar {
        Grammar::JavaScript => vec![
            source.to_vec(),
            blank_ampersands(source),
            blank_variance(&blank_ampersands(source)),
        ],
        Grammar::TypeScript => vec![blank_variance(source)],
        Grammar::Tsx => vec![
            blank_ampersands(source),
            blank_variance(&blank_ampersands(source)),
        ],
    };
    attempts.into_iter().find_map(|attempt| {
        let text = String::from_utf8(attempt).ok()?;
        parse_snippet(fallback, &text).filter(|tree| {
            !tree.root_node().has_error()
                && (grammar != Grammar::JavaScript || !has_expression_type_arguments(tree.root_node()))
        })
    })
}

fn has_expression_type_arguments(root: Node<'_>) -> bool {
    descendants(root).into_iter().any(|node| {
        node.kind() == TYPE_ARGUMENTS
            && node
                .parent()
                .is_some_and(|parent| EXPRESSIONS_WITH_TYPE_ARGUMENTS.contains(&parent.kind()))
    })
}

pub fn is_flow_syntax(node: Node<'_>) -> bool {
    match node.kind() {
        PARENTHESIZED => named_children(node)
            .iter()
            .any(|child| child.kind() == TYPE_ONLY_KIND),
        "import_statement" => has_token(node, TYPEOF),
        "constraint" => first_token(node).is_some_and(|token| token.kind() == COLON),
        "object_type" => first_token(node).is_some_and(|token| token.kind() == EXACT_OBJECT_OPENING),
        "ambient_declaration" => {
            has_token(node, MODULE) && named_children(node).iter().any(|child| child.kind() == PROPERTY)
        }
        _ => false,
    }
}

fn first_token(node: Node<'_>) -> Option<Node<'_>> {
    node.child(0)
}

fn grammar_of(root: Node<'_>) -> Grammar {
    let language = root.language();
    if language.id_for_node_kind(TYPE_ONLY_KIND, true) == 0 {
        Grammar::JavaScript
    } else if language.id_for_node_kind(JSX_KIND, true) == 0 {
        Grammar::TypeScript
    } else {
        Grammar::Tsx
    }
}

fn blank_ampersands(source: &[u8]) -> Vec<u8> {
    let mut repaired = source.to_vec();
    for (index, byte) in source.iter().enumerate() {
        if *byte != AMPERSAND {
            continue;
        }
        let previous = index.checked_sub(1).and_then(|before| source.get(before));
        let next = source.get(index.saturating_add(1));
        let is_operator = previous == Some(&AMPERSAND) || [Some(&AMPERSAND), Some(&EQUALS)].contains(&next);
        let is_reference = CHARACTER_REFERENCE
            .as_ref()
            .is_some_and(|pattern| pattern.is_match(source.get(index..).unwrap_or_default()));
        if !is_operator
            && !is_reference
            && let Some(slot) = repaired.get_mut(index)
        {
            *slot = BLANK;
        }
    }
    repaired
}

fn blank_variance(source: &[u8]) -> Vec<u8> {
    let mut repaired = source.to_vec();
    let Some(pattern) = VARIANCE.as_ref() else {
        return repaired;
    };
    for captures in pattern.captures_iter(source) {
        let Some(modifiers) = captures.get(MODIFIERS_GROUP) else {
            continue;
        };
        if let Some(span) = repaired.get_mut(modifiers.range()) {
            span.fill(BLANK);
        }
    }
    repaired
}
