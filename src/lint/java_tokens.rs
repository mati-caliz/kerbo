use tree_sitter::{Node, Point};

use super::java::children;

const ATOMIC: &[&str] = &["string_literal", "character_literal"];
const STATEMENT_LISTS: &[&str] = &[
    "block",
    "constructor_body",
    "switch_block_statement_group",
    "labeled_statement",
];
const BODY_FIELDS: &[&str] = &["body", "consequence", "alternative"];
const SEMICOLON: &str = ";";
const COMMENTS: &[&str] = &["line_comment", "block_comment"];
const LINE_FEED: u8 = b'\n';
const CARRIAGE_RETURN: u8 = b'\r';

// Una hoja del árbol (o un literal de texto entero) con su padre y su abuelo, anotados al bajar:
// subir al padre en tree-sitter es caro. Lo que tree-sitter no pudo parsear (un ERROR, como una
// anotación sobre `...`) no se mira: sus hojas no dicen qué signo es cuál.
#[derive(Clone, Copy)]
pub struct Token<'tree> {
    pub node: Node<'tree>,
    pub parent: Node<'tree>,
    pub grandparent: Option<Node<'tree>>,
}

impl<'tree> Token<'tree> {
    pub fn kind(&self) -> &'tree str {
        self.node.kind()
    }

    pub fn parent_kind(&self) -> &'tree str {
        self.parent.kind()
    }

    pub fn grandparent_kind(&self) -> &'tree str {
        self.grandparent.map_or("", |grandparent| grandparent.kind())
    }

    pub fn start(&self) -> Point {
        self.node.start_position()
    }

    pub fn end(&self) -> Point {
        self.node.end_position()
    }

    pub fn is_comment(&self) -> bool {
        COMMENTS.contains(&self.kind())
    }

    // Checkstyle llama EMPTY_STAT al `;` que hace de sentencia y SEMI a los demás.
    pub fn is_empty_statement(&self) -> bool {
        self.kind() == SEMICOLON
            && (STATEMENT_LISTS.contains(&self.parent_kind())
                || BODY_FIELDS
                    .iter()
                    .any(|field| self.parent.child_by_field_name(field) == Some(self.node)))
    }
}

pub fn tokens(root: Node<'_>) -> Vec<Token<'_>> {
    let mut found = Vec::new();
    let mut pending: Vec<(Node<'_>, Option<Node<'_>>, Option<Node<'_>>)> = vec![(root, None, None)];
    while let Some((node, parent, grandparent)) = pending.pop() {
        if node.is_error() {
            continue;
        }
        if node.child_count() == 0 || ATOMIC.contains(&node.kind()) {
            if let Some(parent) = parent.filter(|_| node.end_byte() > node.start_byte()) {
                found.push(Token {
                    node,
                    parent,
                    grandparent,
                });
            }
            continue;
        }
        pending.extend(
            children(node)
                .into_iter()
                .rev()
                .map(|child| (child, Some(node), parent)),
        );
    }
    found
}

// Las líneas del archivo sin el fin de línea, como las ve Checkstyle.
pub struct Lines<'source> {
    lines: Vec<&'source [u8]>,
}

impl<'source> Lines<'source> {
    pub fn new(source: &'source [u8]) -> Self {
        Self {
            lines: source
                .split(|byte| *byte == LINE_FEED)
                .map(|line| line.strip_suffix(&[CARRIAGE_RETURN]).unwrap_or(line))
                .collect(),
        }
    }

    pub fn byte(&self, row: usize, column: usize) -> Option<u8> {
        self.lines.get(row).and_then(|line| line.get(column)).copied()
    }

    pub fn width(&self, row: usize) -> usize {
        self.lines.get(row).map_or(0, |line| line.len())
    }

    pub fn is_blank_before(&self, at: Point) -> bool {
        self.lines
            .get(at.row)
            .and_then(|line| line.get(..at.column))
            .is_none_or(|before| before.iter().all(|byte| is_whitespace(*byte)))
    }

    pub fn whitespace_before(&self, at: Point) -> bool {
        at.column
            .checked_sub(1)
            .and_then(|column| self.byte(at.row, column))
            .is_some_and(is_whitespace)
    }

    pub fn whitespace_at(&self, at: Point) -> bool {
        self.byte(at.row, at.column).is_some_and(is_whitespace)
    }

    // Dónde empieza el blanco que termina en `at`, sin salir de la línea.
    pub fn blank_start(&self, at: Point) -> Point {
        let mut column = at.column;
        while column > 0
            && self
                .byte(at.row, column.saturating_sub(1))
                .is_some_and(is_whitespace)
        {
            column = column.saturating_sub(1);
        }
        Point::new(at.row, column)
    }

    // Dónde termina el blanco que empieza en `at`, sin salir de la línea.
    pub fn blank_end(&self, at: Point) -> Point {
        let mut column = at.column;
        while self.byte(at.row, column).is_some_and(is_whitespace) {
            column = column.saturating_add(1);
        }
        Point::new(at.row, column)
    }
}

pub const fn is_whitespace(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | 0x0b | 0x0c)
}
