use tree_sitter::Node;

use super::{
    ELEMENT_COMPREHENSIONS, FOR_IN_CLAUSE, GENERATOR, GroupFinder, NAMED_EXPRESSION, PAIR, PARAMETERS_FIELD,
    significant,
};
use crate::lint::python::ast::{
    ATTRIBUTE, BODY_FIELD, IDENTIFIER, KEYWORD_ARGUMENT, LAMBDA, LEFT_FIELD, NAME_FIELD, OBJECT_FIELD,
    RIGHT_FIELD, VALUE_FIELD,
};

impl<'tree> GroupFinder<'_, 'tree> {
    pub(super) fn expression(&mut self, node: Node<'tree>) {
        if node.kind() == NAMED_EXPRESSION
            && node
                .child_by_field_name(NAME_FIELD)
                .is_some_and(|name| self.is_group(name))
        {
            self.overridden = true;
        }
        if self.overridden {
            return;
        }
        match node.kind() {
            kind if ELEMENT_COMPREHENSIONS.contains(&kind) => {
                self.comprehension_clauses(node);
                if !self.overridden {
                    self.nested = true;
                    self.comprehension_element(node.child_by_field_name(BODY_FIELD));
                    self.nested = false;
                }
            }
            GENERATOR => {
                self.comprehension_clauses(node);
                if let Some(element) = node.child_by_field_name(BODY_FIELD) {
                    self.expression(element);
                }
            }
            IDENTIFIER if self.is_group(node) => self.usage(node),
            KEYWORD_ARGUMENT => self.field(node, VALUE_FIELD),
            ATTRIBUTE => self.field(node, OBJECT_FIELD),
            LAMBDA => {
                if let Some(parameters) = node.child_by_field_name(PARAMETERS_FIELD) {
                    self.parameters(parameters);
                }
                self.field(node, BODY_FIELD);
            }
            _ => {
                for child in significant(node) {
                    self.expression(child);
                }
            }
        }
    }

    // ruff recorre los hijos del elemento sin mirar el elemento mismo: `[grupo for x in xs]` no cuenta,
    // `[f(grupo) for x in xs]` sí. En un diccionario, lo mismo con la clave y con el valor.
    pub(super) fn comprehension_element(&mut self, element: Option<Node<'tree>>) {
        let parts = match element {
            Some(pair) if pair.kind() == PAIR => significant(pair),
            Some(element) => vec![element],
            None => Vec::new(),
        };
        for part in parts {
            self.walk(part);
        }
    }

    pub(super) fn walk(&mut self, node: Node<'tree>) {
        match node.kind() {
            IDENTIFIER => {}
            ATTRIBUTE => self.field(node, OBJECT_FIELD),
            _ => {
                for child in significant(node) {
                    self.expression(child);
                }
            }
        }
    }

    pub(super) fn field(&mut self, node: Node<'tree>, field: &str) {
        if let Some(child) = node.child_by_field_name(field) {
            self.expression(child);
        }
    }

    // Un uso suelto: avisa si ya se usó antes o si está adentro de un loop.
    pub(super) fn usage(&mut self, node: Node<'tree>) {
        self.increment(1);
        let pending: u32 = self
            .branches
            .iter()
            .map(|branch| branch.last().copied().unwrap_or(0))
            .sum();
        if self.nested || self.usage_count.saturating_add(pending) > 1 {
            self.found.push(node);
        }
    }

    // Las cláusulas `for` de una comprensión: sólo el destino y el iterable, sin las condiciones.
    pub(super) fn comprehension_clauses(&mut self, comprehension: Node<'tree>) {
        for clause in significant(comprehension)
            .into_iter()
            .filter(|clause| clause.kind() == FOR_IN_CLAUSE)
        {
            if clause
                .child_by_field_name(LEFT_FIELD)
                .is_some_and(|target| self.is_group(target))
            {
                self.overridden = true;
            }
            if self.overridden {
                return;
            }
            self.iterated(clause.child_by_field_name(RIGHT_FIELD));
        }
    }
}
