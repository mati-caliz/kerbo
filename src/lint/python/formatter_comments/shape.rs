use tree_sitter::Node;

use crate::lint::python::ast::{
    ASSIGNMENT, AUGMENTED_ASSIGNMENT, BLOCK, COMMENT, DECORATED, EXPRESSION_STATEMENT, IDENTIFIER,
    PARENTHESIZED, STRING, named,
};

const TYPE_NODE: &str = "type";
const CASE_PATTERN: &str = "case_pattern";
const IF_STATEMENT: &str = "if_statement";
const FOR_STATEMENT: &str = "for_statement";
const WHILE_STATEMENT: &str = "while_statement";
const TRY_STATEMENT: &str = "try_statement";
const MATCH_STATEMENT: &str = "match_statement";
const ELIF_CLAUSE: &str = "elif_clause";
const ELSE_CLAUSE: &str = "else_clause";
const FINALLY_CLAUSE: &str = "finally_clause";
const EXCEPT_CLAUSES: [&str; 2] = ["except_clause", "except_group_clause"];
const STATEMENT_SUFFIX: &str = "_statement";
const DEFINITION_FIELD: &str = "definition";
const DECORATOR: &str = "decorator";
const SKIPPED_FIELDS: [&str; 3] = ["name", "attribute", "alias"];
const WITHOUT_ITEMS: [&str; 4] = [STRING, "global_statement", "nonlocal_statement", IDENTIFIER];
const FLATTENED: [&str; 2] = ["pair", "with_clause"];
// Los nodos que en el AST de ruff son sentencias (o partes de una, como un `except` o un `case`).
const STATEMENT_PARTS: [&str; 11] = [
    DECORATED,
    "function_definition",
    "class_definition",
    DECORATOR,
    ASSIGNMENT,
    AUGMENTED_ASSIGNMENT,
    "except_clause",
    "except_group_clause",
    "case_clause",
    ELIF_CLAUSE,
    ELSE_CLAUSE,
];

// Un hijo en el recorrido de ruff: un nodo, o el cuerpo de una sentencia (que ruff recorre aparte).
pub enum Item<'tree> {
    Node(Node<'tree>),
    Body(Vec<Node<'tree>>),
}

pub fn is_statement(node: Node<'_>) -> bool {
    node.kind().ends_with(STATEMENT_SUFFIX) || STATEMENT_PARTS.contains(&node.kind())
}

// El nodo de ruff que corresponde a uno de tree-sitter: sin paréntesis ni envoltorios, y la
// asignación en vez de la sentencia que la contiene.
pub fn shaped(node: Node<'_>) -> Node<'_> {
    let children = || -> Vec<Node<'_>> {
        named(node)
            .into_iter()
            .filter(|child| child.kind() != COMMENT)
            .collect()
    };
    match node.kind() {
        PARENTHESIZED | TYPE_NODE | CASE_PATTERN => match children().as_slice() {
            [only] => shaped(*only),
            _ => node,
        },
        EXPRESSION_STATEMENT => match children().as_slice() {
            [only] if matches!(only.kind(), ASSIGNMENT | AUGMENTED_ASSIGNMENT) => *only,
            _ => node,
        },
        _ => node,
    }
}

// Las sentencias de un bloque o del módulo.
pub fn statements(body: Node<'_>) -> Vec<Node<'_>> {
    named(body)
        .into_iter()
        .filter(|child| child.kind() != COMMENT)
        .map(shaped)
        .collect()
}

// Los hijos de un nodo en el orden en que los visita ruff, sin los nombres (sus `Identifier`).
pub fn items(node: Node<'_>) -> Vec<Item<'_>> {
    if WITHOUT_ITEMS.contains(&node.kind()) {
        return Vec::new();
    }
    if node.kind() == DECORATED {
        let mut decorated: Vec<Item<'_>> = named(node)
            .into_iter()
            .filter(|child| child.kind() == DECORATOR)
            .map(Item::Node)
            .collect();
        decorated.extend(
            node.child_by_field_name(DEFINITION_FIELD)
                .map(items)
                .unwrap_or_default(),
        );
        return decorated;
    }
    let skipped: Vec<usize> = SKIPPED_FIELDS
        .iter()
        .filter_map(|field| node.child_by_field_name(field))
        .filter(|child| child.kind() == IDENTIFIER)
        .map(|child| child.id())
        .collect();
    let mut items = Vec::new();
    for child in named(node) {
        if child.kind() == COMMENT || skipped.contains(&child.id()) {
            continue;
        }
        match child.kind() {
            BLOCK if node.kind() == MATCH_STATEMENT => {
                items.extend(statements(child).into_iter().map(Item::Node));
            }
            BLOCK => items.push(Item::Body(statements(child))),
            ELSE_CLAUSE | FINALLY_CLAUSE if node.kind() != IF_STATEMENT => {
                items.push(Item::Body(clause_statements(child)));
            }
            kind if FLATTENED.contains(&kind) => items.extend(self::items(child)),
            _ => items.push(Item::Node(shaped(child))),
        }
    }
    items
}

fn clause_statements(clause: Node<'_>) -> Vec<Node<'_>> {
    named(clause)
        .into_iter()
        .find(|child| child.kind() == BLOCK)
        .map(statements)
        .unwrap_or_default()
}

// `is_first_statement_in_alternate_body`: el primer `elif`/`else` de un `if`, el primer `except` de
// un `try`, o la primera sentencia de un `else` o `finally`.
pub fn is_first_in_alternate_body(following: Node<'_>, enclosing: Node<'_>) -> bool {
    let clauses = named(enclosing);
    let first_of = |kinds: &[&str]| {
        clauses
            .iter()
            .find(|clause| kinds.contains(&clause.kind()))
            .copied()
    };
    let first_statement = |kind: &str| {
        first_of(&[kind])
            .and_then(|clause| clause_statements(clause).first().copied())
            .is_some_and(|statement| statement.id() == following.id())
    };
    match enclosing.kind() {
        IF_STATEMENT => {
            first_of(&[ELIF_CLAUSE, ELSE_CLAUSE]).is_some_and(|clause| clause.id() == following.id())
        }
        FOR_STATEMENT | WHILE_STATEMENT => first_statement(ELSE_CLAUSE),
        TRY_STATEMENT => {
            first_of(&EXCEPT_CLAUSES).is_some_and(|clause| clause.id() == following.id())
                || first_statement(ELSE_CLAUSE)
                || first_statement(FINALLY_CLAUSE)
        }
        _ => false,
    }
}
