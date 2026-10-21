use tree_sitter::Node;

use super::model::BindingKind;
use super::targets::Store;
use super::{Mode, Visitor};
use crate::lint::python::ast::{BLOCK, BODY_FIELD, DOTTED_NAME, IDENTIFIER, fields, named};
use crate::lint::python::semantic::expressions::Expressions;
use crate::lint::python::semantic::statements::Statements;
use crate::lint::python::semantic::targets::Targets;

const SUBJECT_FIELD: &str = "subject";
const ALTERNATIVE_FIELD: &str = "alternative";
const CONSEQUENCE_FIELD: &str = "consequence";
const GUARD_FIELD: &str = "guard";
const CASE_PATTERN: &str = "case_pattern";
const AS_PATTERN: &str = "as_pattern";
const KEYWORD_PATTERN: &str = "keyword_pattern";
const SPLAT_PATTERN: &str = "splat_pattern";
const CLASS_PATTERN: &str = "class_pattern";
const DICT_PATTERN: &str = "dict_pattern";
const WILDCARD: &str = "_";

#[derive(Debug, Default)]
struct Parts<'tree> {
    values: Vec<Node<'tree>>,
    nested: Vec<Node<'tree>>,
    captures: Vec<Node<'tree>>,
}

pub trait Patterns<'tree> {
    fn match_statement(&mut self, statement: Node<'tree>);
    fn pattern(&mut self, pattern: Node<'tree>, statement: Node<'tree>);
    fn value_pattern(&mut self, pattern: Node<'tree>);
    fn capture(&mut self, name: Node<'tree>, statement: Node<'tree>);
}

impl<'tree> Patterns<'tree> for Visitor<'tree> {
    fn match_statement(&mut self, statement: Node<'tree>) {
        for subject in fields(statement, SUBJECT_FIELD) {
            self.expression(subject, Mode::Value);
        }
        let cases = statement
            .child_by_field_name(BODY_FIELD)
            .filter(|body| body.kind() == BLOCK)
            .map(|body| fields(body, ALTERNATIVE_FIELD))
            .unwrap_or_default();
        for case in cases {
            self.model.push_branch();
            for pattern in named(case)
                .into_iter()
                .filter(|child| child.kind() == CASE_PATTERN)
            {
                self.pattern(pattern, statement);
            }
            if let Some(guard) = case.child_by_field_name(GUARD_FIELD) {
                self.loads(guard);
            }
            if let Some(consequence) = case.child_by_field_name(CONSEQUENCE_FIELD) {
                self.block(consequence);
            }
            self.model.pop_branch();
        }
    }

    fn pattern(&mut self, pattern: Node<'tree>, statement: Node<'tree>) {
        let parts = parts_of(pattern);
        for value in parts.values {
            self.value_pattern(value);
        }
        for nested in parts.nested {
            self.pattern(nested, statement);
        }
        for capture in parts.captures {
            self.capture(capture, statement);
        }
    }

    fn value_pattern(&mut self, pattern: Node<'tree>) {
        let loaded = if pattern.kind() == DOTTED_NAME {
            named(pattern).first().copied()
        } else {
            Some(pattern)
        };
        if let Some(loaded) = loaded {
            self.expression(loaded, Mode::Value);
        }
    }

    fn capture(&mut self, name: Node<'tree>, statement: Node<'tree>) {
        if name.kind() == IDENTIFIER && self.text(name) != WILDCARD {
            self.bind_outside_branch(name, Store::plain(BindingKind::Assignment, statement));
        }
    }
}

fn parts_of(pattern: Node<'_>) -> Parts<'_> {
    let children = named(pattern);
    match pattern.kind() {
        DOTTED_NAME if children.len() == 1 => Parts {
            captures: children,
            ..Parts::default()
        },
        DOTTED_NAME => Parts {
            values: vec![pattern],
            ..Parts::default()
        },
        AS_PATTERN => {
            let mut nested = children;
            let captures = nested.pop().into_iter().collect();
            Parts {
                nested,
                captures,
                ..Parts::default()
            }
        }
        KEYWORD_PATTERN => Parts {
            nested: children.into_iter().skip(1).collect(),
            ..Parts::default()
        },
        SPLAT_PATTERN | IDENTIFIER => Parts {
            captures: if pattern.kind() == IDENTIFIER {
                vec![pattern]
            } else {
                children
            },
            ..Parts::default()
        },
        CLASS_PATTERN => {
            let mut nested = children;
            let values = if nested.is_empty() {
                Vec::new()
            } else {
                vec![nested.remove(0)]
            };
            Parts {
                values,
                nested,
                ..Parts::default()
            }
        }
        DICT_PATTERN => {
            let (nested, values) = children
                .into_iter()
                .partition(|part| matches!(part.kind(), CASE_PATTERN | SPLAT_PATTERN));
            Parts {
                values,
                nested,
                ..Parts::default()
            }
        }
        _ => Parts {
            nested: children,
            ..Parts::default()
        },
    }
}
