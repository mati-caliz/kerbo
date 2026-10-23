use tree_sitter::Node;

use super::directives::Directives;
use super::model::SourceText;
use crate::lint::python::ast::{BLOCK, CLASS, COMMENT, DECORATED, FUNCTION, IMPORT, IMPORT_FROM, named};

const FUTURE_IMPORT: &str = "future_import_statement";
const DEFINITION_FIELD: &str = "definition";
const BODY_FIELD: &str = "body";
const CONSEQUENCE_FIELD: &str = "consequence";
const ALTERNATIVE_FIELD: &str = "alternative";
const FOR: &str = "for_statement";
const WHILE: &str = "while_statement";
const IF: &str = "if_statement";
const WITH: &str = "with_statement";
const MATCH: &str = "match_statement";
const TRY: &str = "try_statement";
const CASE: &str = "case_clause";
const EXCEPT_CLAUSES: [&str; 2] = ["except_clause", "except_group_clause"];
const TRY_TAILS: [&str; 2] = ["else_clause", "finally_clause"];
const HASH: &str = "#";
const DECORATOR_MARK: &str = "@";

// Lo que viene después de un bloque de imports del módulo, que decide cuántas líneas en blanco
// pide ruff.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trailer {
    Sibling,
    Definition,
}

#[derive(Debug, Default)]
pub struct ImportGroup<'tree> {
    pub imports: Vec<Node<'tree>>,
    pub trailer: Option<Trailer>,
}

// `BlockBuilder` de ruff: junta los imports seguidos y corta en cada otra sentencia, en cada
// `# isort: split` y al entrar o salir de un bloque anidado.
pub struct Builder<'tree, 'source> {
    groups: Vec<ImportGroup<'tree>>,
    directives: Directives,
    text_source: SourceText<'source>,
    nested: bool,
    next_split: usize,
}

impl<'tree, 'source> Builder<'tree, 'source> {
    pub fn new(directives: Directives, text_source: SourceText<'source>) -> Self {
        Self {
            groups: vec![ImportGroup::default()],
            directives,
            text_source,
            nested: false,
            next_split: 0,
        }
    }

    pub fn into_groups(self) -> Vec<ImportGroup<'tree>> {
        self.groups
    }

    pub fn visit_body(&mut self, body: Option<Node<'tree>>) {
        for statement in body
            .map(named)
            .unwrap_or_default()
            .into_iter()
            .filter(|child| child.kind() != COMMENT)
        {
            self.visit(statement);
        }
    }

    fn visit_clause(&mut self, clause: Node<'tree>) {
        let body = clause
            .child_by_field_name(CONSEQUENCE_FIELD)
            .or_else(|| clause.child_by_field_name(BODY_FIELD))
            .or_else(|| named(clause).into_iter().find(|child| child.kind() == BLOCK));
        self.visit_body(body);
        self.finalize(None);
    }

    fn visit(&mut self, statement: Node<'tree>) {
        if self.passes_split(statement.start_byte()) {
            let trailer = self.trailer_for(statement);
            self.finalize(trailer);
        }
        let is_import = matches!(statement.kind(), IMPORT | IMPORT_FROM | FUTURE_IMPORT);
        if is_import && !self.directives.excludes(statement.start_byte()) {
            if let Some(group) = self.groups.last_mut() {
                group.imports.push(statement);
            }
        } else {
            let trailer = self.trailer_for(statement);
            self.finalize(trailer);
        }
        let previous = self.nested;
        self.nested = true;
        self.visit_nested(statement);
        self.nested = previous;
    }

    fn visit_nested(&mut self, statement: Node<'tree>) {
        let definition = if statement.kind() == DECORATED {
            statement
                .child_by_field_name(DEFINITION_FIELD)
                .unwrap_or(statement)
        } else {
            statement
        };
        match definition.kind() {
            FUNCTION | CLASS | WITH | FOR | WHILE => {
                self.visit_body(definition.child_by_field_name(BODY_FIELD));
                self.finalize(None);
                for clause in alternatives(definition) {
                    self.visit_clause(clause);
                }
            }
            IF => {
                self.visit_body(definition.child_by_field_name(CONSEQUENCE_FIELD));
                self.finalize(None);
                for clause in alternatives(definition) {
                    self.visit_clause(clause);
                }
            }
            MATCH => {
                let cases = definition
                    .child_by_field_name(BODY_FIELD)
                    .map(named)
                    .unwrap_or_default();
                for case in cases.into_iter().filter(|case| case.kind() == CASE) {
                    self.visit_clause(case);
                }
            }
            TRY => self.visit_try(definition),
            _ => {}
        }
    }

    // Como ruff, primero los `except` y después el cuerpo, el `else` y el `finally`.
    fn visit_try(&mut self, statement: Node<'tree>) {
        let clauses = named(statement);
        for handler in clauses
            .iter()
            .filter(|clause| EXCEPT_CLAUSES.contains(&clause.kind()))
        {
            let previous = self.nested;
            self.nested = true;
            self.visit_clause(*handler);
            self.nested = previous;
        }
        self.visit_body(statement.child_by_field_name(BODY_FIELD));
        self.finalize(None);
        for tail in clauses.iter().filter(|clause| TRY_TAILS.contains(&clause.kind())) {
            self.visit_clause(*tail);
        }
    }

    fn passes_split(&mut self, start: usize) -> bool {
        let splits = &self.directives.splits;
        let mut passed = false;
        while splits.get(self.next_split).is_some_and(|split| start >= *split) {
            self.next_split = self.next_split.saturating_add(1);
            passed = true;
        }
        passed
    }

    fn trailer_for(&self, statement: Node<'tree>) -> Option<Trailer> {
        let has_imports = self.groups.last().is_some_and(|group| !group.imports.is_empty());
        if !has_imports || self.nested {
            return None;
        }
        let definition = if statement.kind() == DECORATED {
            statement
                .child_by_field_name(DEFINITION_FIELD)
                .unwrap_or(statement)
        } else {
            statement
        };
        let is_definition = matches!(definition.kind(), FUNCTION | CLASS);
        Some(
            if is_definition && !self.has_comment_break(statement.start_byte()) {
                Trailer::Definition
            } else {
                Trailer::Sibling
            },
        )
    }

    // `has_comment_break`: un comentario suelto, separado por una línea en blanco, entre los
    // imports y la definición.
    fn has_comment_break(&self, start: usize) -> bool {
        let before = self.text_source.slice(0, self.text_source.line_start(start));
        let mut seen_blank = false;
        for line in before.lines().rev().map(str::trim) {
            let is_comment = line.starts_with(HASH);
            if seen_blank && !line.is_empty() {
                return is_comment;
            }
            if line.is_empty() {
                seen_blank = true;
            } else if !is_comment && !line.starts_with(DECORATOR_MARK) {
                return false;
            }
        }
        false
    }

    fn finalize(&mut self, trailer: Option<Trailer>) {
        if let Some(group) = self.groups.last_mut().filter(|group| !group.imports.is_empty()) {
            group.trailer = trailer;
            self.groups.push(ImportGroup::default());
        }
    }
}

fn alternatives(statement: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = statement.walk();
    statement
        .children_by_field_name(ALTERNATIVE_FIELD, &mut cursor)
        .collect()
}
