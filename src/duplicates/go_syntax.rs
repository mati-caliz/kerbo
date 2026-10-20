use tree_sitter::Node;

use super::go_kinds::{
    ARRAY_TYPE, BASIC_LIT, BINARY_EXPR, CALL_EXPR, CHAN_TYPE, IDENT, INDEX_EXPR, KEY_VALUE_EXPR, MAP_TYPE,
    PAREN_EXPR, SELECTOR_EXPR, SLICE_EXPR, STAR_EXPR, TYPE_ASSERT_EXPR, UNARY_EXPR,
};
use super::{go_statements, go_types};
use crate::syntax::named_children;

const MAX_SERIALIZED_CHILDREN: usize = 10_001;
const UNKNOWN_NODE: i32 = 0;
const IDENTIFIER_KINDS: &[&str] = &[
    "identifier",
    "field_identifier",
    "type_identifier",
    "package_identifier",
    "label_name",
    "blank_identifier",
    "true",
    "false",
    "nil",
    "iota",
];
const LITERAL_KINDS: &[&str] = &[
    "int_literal",
    "float_literal",
    "imaginary_literal",
    "rune_literal",
    "interpreted_string_literal",
    "raw_string_literal",
];
const PAIRS: &[(&str, i32, &str, &str)] = &[
    ("selector_expression", SELECTOR_EXPR, "operand", "field"),
    ("qualified_type", SELECTOR_EXPR, "package", "name"),
    ("index_expression", INDEX_EXPR, "operand", "index"),
    ("binary_expression", BINARY_EXPR, "left", "right"),
    ("map_type", MAP_TYPE, "key", "value"),
    ("keyed_element", KEY_VALUE_EXPR, "key", "value"),
    ("type_assertion_expression", TYPE_ASSERT_EXPR, "operand", "type"),
    ("type_conversion_expression", CALL_EXPR, "type", "operand"),
    ("array_type", ARRAY_TYPE, "length", "element"),
];
const WRAPPERS: &[(&str, i32)] = &[
    ("parenthesized_expression", PAREN_EXPR),
    ("parenthesized_type", PAREN_EXPR),
    ("pointer_type", STAR_EXPR),
    ("negated_type", UNARY_EXPR),
    ("slice_type", ARRAY_TYPE),
    ("channel_type", CHAN_TYPE),
];
const COMMENT_KIND: &str = "comment";
const DEREFERENCE: &str = "*";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyntaxNode {
    pub kind: i32,
    pub start: usize,
    pub end: usize,
    pub owns: usize,
    pub line: usize,
    pub end_line: usize,
}

#[derive(Debug)]
pub struct Builder<'source> {
    source: &'source [u8],
    newlines: Vec<usize>,
    nodes: Vec<SyntaxNode>,
    child_counts: Vec<usize>,
}

// dupl recorre el árbol de go/ast, no el de tree-sitter: cada nodo se traduce al que armaría
// go/parser, con sus hijos en el mismo orden (AssignStmt pone primero el lado derecho) y su
// misma posición, porque las líneas que reporta salen del primer y del último nodo del bloque.
pub fn nodes(root: Node<'_>, source: &[u8]) -> Option<Vec<SyntaxNode>> {
    if root.has_error() {
        return None;
    }
    let mut builder = Builder {
        source,
        newlines: source
            .iter()
            .enumerate()
            .filter(|&(_, &byte)| byte == b'\n')
            .map(|(offset, _)| offset)
            .collect(),
        nodes: Vec::new(),
        child_counts: Vec::new(),
    };
    go_statements::source_file(&mut builder, root);
    Some(builder.nodes)
}

pub fn significant_children(node: Node<'_>) -> Vec<Node<'_>> {
    named_children(node)
        .into_iter()
        .filter(|child| child.kind() != COMMENT_KIND)
        .collect()
}

pub fn token<'tree>(node: Node<'tree>, text: &str) -> Option<Node<'tree>> {
    (0..node.child_count())
        .filter_map(|index| node.child(index))
        .find(|child| !child.is_named() && child.kind() == text)
}

pub fn token_start(node: Node<'_>, text: &str) -> usize {
    token(node, text).map_or_else(|| node.start_byte(), |found| found.start_byte())
}

pub fn field<'tree>(node: Node<'tree>, name: &str) -> Option<Node<'tree>> {
    node.child_by_field_name(name)
}

pub fn span(node: Node<'_>) -> (usize, usize) {
    (node.start_byte(), node.end_byte())
}

pub fn signature_span(node: Node<'_>, start: usize) -> (usize, usize) {
    let end = field(node, "result")
        .or_else(|| field(node, "parameters"))
        .map_or_else(|| node.end_byte(), |last| last.end_byte());
    (start, end)
}

impl Builder<'_> {
    fn line_of(&self, offset: usize) -> usize {
        self.newlines
            .partition_point(|&newline| newline <= offset)
            .saturating_add(1)
    }

    fn has_room(&mut self) -> bool {
        let Some(count) = self.child_counts.last_mut() else {
            return true;
        };
        if *count >= MAX_SERIALIZED_CHILDREN {
            return false;
        }
        *count = count.saturating_add(1);
        true
    }

    pub fn wrap(&mut self, kind: i32, (start, end): (usize, usize), body: impl FnOnce(&mut Self)) {
        if !self.has_room() {
            return;
        }
        let index = self.nodes.len();
        self.nodes.push(SyntaxNode {
            kind,
            start,
            end,
            owns: 0,
            line: self.line_of(start),
            end_line: self.line_of(end.saturating_sub(1)),
        });
        self.child_counts.push(0);
        body(self);
        self.child_counts.pop();
        let owns = self.nodes.len().saturating_sub(index).saturating_sub(1);
        if let Some(node) = self.nodes.get_mut(index) {
            node.owns = owns;
        }
    }

    pub fn leaf(&mut self, kind: i32, node: Node<'_>) {
        self.wrap(kind, span(node), |_| {});
    }

    pub fn optional(&mut self, node: Option<Node<'_>>) {
        if let Some(present) = node {
            self.expression(present);
        }
    }

    pub fn first_child(&mut self, node: Node<'_>) {
        self.optional(significant_children(node).first().copied());
    }

    pub fn expressions(&mut self, list: Option<Node<'_>>) {
        for item in list.map(significant_children).unwrap_or_default() {
            self.expression(item);
        }
    }

    pub fn expression(&mut self, node: Node<'_>) {
        let kind = node.kind();
        if IDENTIFIER_KINDS.contains(&kind) {
            return self.leaf(IDENT, node);
        }
        if LITERAL_KINDS.contains(&kind) {
            return self.leaf(BASIC_LIT, node);
        }
        if let Some(&(_, wrapped, first, second)) = PAIRS.iter().find(|(name, ..)| *name == kind) {
            return self.wrap(wrapped, span(node), |inner| {
                inner.optional(field(node, first));
                inner.optional(field(node, second));
            });
        }
        if let Some(&(_, wrapped)) = WRAPPERS.iter().find(|(name, _)| *name == kind) {
            return self.wrap(wrapped, span(node), |inner| inner.first_child(node));
        }
        match kind {
            "literal_element" | "variadic_argument" => self.first_child(node),
            "type_elem" | "type_constraint" => self.union(&significant_children(node)),
            "unary_expression" => self.unary(node),
            "slice_expression" => self.slice(node),
            _ => go_types::composite_expression(self, node),
        }
    }

    pub fn unknown(&mut self, node: Node<'_>) {
        self.leaf(UNKNOWN_NODE, node);
    }

    fn unary(&mut self, node: Node<'_>) {
        let operator = field(node, "operator").and_then(|operator| operator.utf8_text(self.source).ok());
        let kind = if operator == Some(DEREFERENCE) {
            STAR_EXPR
        } else {
            UNARY_EXPR
        };
        self.wrap(kind, span(node), |inner| inner.optional(field(node, "operand")));
    }

    fn slice(&mut self, node: Node<'_>) {
        self.wrap(SLICE_EXPR, span(node), |inner| {
            for name in ["operand", "start", "end", "capacity"] {
                inner.optional(field(node, name));
            }
        });
    }

    pub fn union(&mut self, types: &[Node<'_>]) {
        let (Some((&last, rest)), Some(&first)) = (types.split_last(), types.first()) else {
            return;
        };
        if rest.is_empty() {
            return self.expression(last);
        }
        self.wrap(BINARY_EXPR, (first.start_byte(), last.end_byte()), |inner| {
            inner.union(rest);
            inner.expression(last);
        });
    }
}
