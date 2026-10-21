use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::hash_map::Entry;

use std::sync::LazyLock;

use regex::bytes::Regex;
use tree_sitter::{Node, Parser, Tree};

use crate::profile::{Kinds, Profile};

pub const ANONYMOUS_FUNCTION_IDENTITY: &str = "(anónima)";
const NAME_FIELD: &str = "name";
const PRIMITIVE_MACRO_ALIAS: &[u8] = b"s7r$bang";
static PRIMITIVE_MACRO: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(r"\bstr(?P<bang>\s*!)").ok());
const NAMING_PARENTS: &[&str] = &[
    "type_alias_declaration",
    "variable_declarator",
    "pair",
    "public_field_definition",
    "assignment",
];

pub fn children_with_fields(node: Node<'_>) -> Vec<(Option<&str>, Node<'_>)> {
    let mut cursor = node.walk();
    let mut children = Vec::new();
    if cursor.goto_first_child() {
        loop {
            children.push((cursor.field_name(), cursor.node()));
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
    children
}

pub fn logical_operator<'source>(
    profile: &Profile,
    node: Node<'_>,
    source: &'source [u8],
    operators: Kinds,
) -> Option<&'source str> {
    if !profile.logical.kinds.contains(&node.kind()) {
        return None;
    }
    let operator = node.child_by_field_name("operator")?.utf8_text(source).ok()?;
    operators.contains(&operator).then_some(operator)
}

pub fn parent_skipping_parentheses<'tree>(profile: &Profile, node: Node<'tree>) -> Option<Node<'tree>> {
    let mut parent = node.parent()?;
    while profile.parenthesized.contains(&parent.kind()) {
        parent = parent.parent()?;
    }
    Some(parent)
}

pub fn function_name(node: Node<'_>, source: &[u8]) -> String {
    let own_name = node.child_by_field_name(NAME_FIELD);
    let inherited_name = || {
        node.parent()
            .filter(|parent| NAMING_PARENTS.contains(&parent.kind()))
            .and_then(|parent| {
                parent
                    .child_by_field_name(NAME_FIELD)
                    .or_else(|| parent.child_by_field_name("left"))
            })
    };
    own_name
        .or_else(inherited_name)
        .and_then(|name| name.utf8_text(source).ok())
        .map_or_else(|| ANONYMOUS_FUNCTION_IDENTITY.to_owned(), str::to_owned)
}

pub fn descendants(root: Node<'_>) -> Vec<Node<'_>> {
    let mut found = Vec::new();
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        found.push(node);
        pending.extend(
            (0..node.child_count())
                .rev()
                .filter_map(|index| node.child(index)),
        );
    }
    found
}

pub fn named_children(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).collect()
}

pub const fn one_based(position: usize) -> usize {
    position.saturating_add(1)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SnippetLanguage {
    Python,
    Go,
    Rust,
    TypeScript,
    Tsx,
}

thread_local! {
    static SNIPPET_PARSERS: RefCell<HashMap<SnippetLanguage, Parser>> = RefCell::new(HashMap::new());
}

// Los comentarios candidatos y los argumentos de macros se compilan aparte; crear un parser por
// fragmento costaba más que analizar el archivo entero.
pub fn parse_snippet(language: SnippetLanguage, text: &str) -> Option<Tree> {
    SNIPPET_PARSERS.with(|parsers| {
        let mut parsers = parsers.borrow_mut();
        let parser = match parsers.entry(language) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let mut parser = Parser::new();
                let grammar = match language {
                    SnippetLanguage::Python => tree_sitter_python::LANGUAGE,
                    SnippetLanguage::Go => tree_sitter_go::LANGUAGE,
                    SnippetLanguage::Rust => tree_sitter_rust::LANGUAGE,
                    SnippetLanguage::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
                    SnippetLanguage::Tsx => tree_sitter_typescript::LANGUAGE_TSX,
                };
                parser.set_language(&grammar.into()).ok()?;
                entry.insert(parser)
            }
        };
        match language {
            SnippetLanguage::Rust => parser.parse(&*rust_parse_text(text.as_bytes()), None),
            SnippetLanguage::Python
            | SnippetLanguage::Go
            | SnippetLanguage::TypeScript
            | SnippetLanguage::Tsx => parser.parse(text, None),
        }
    })
}

// tree-sitter-rust lee `str![...]` (la macro de snapbox) como un índice sobre el tipo `str` con un
// error en el medio. Para parsear se le cambia el nombre por otro identificador del mismo largo:
// las posiciones no se mueven y el nombre se sigue leyendo del texto original.
pub fn rust_parse_text(text: &[u8]) -> Cow<'_, [u8]> {
    PRIMITIVE_MACRO.as_ref().map_or(Cow::Borrowed(text), |pattern| {
        pattern.replace_all(text, PRIMITIVE_MACRO_ALIAS)
    })
}
