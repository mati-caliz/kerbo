use std::collections::HashSet;

use tree_sitter::Node;

use crate::shell::context::Context;
use crate::shell::flow::{integer_assignments, numeric_expansions};
use crate::shell::syntax::{
    ASSIGNMENT, EXPANSIONS, STRING, TEST, VALUE_FIELD, WORD, descendants, named_children,
};
use crate::shell::test_expression::{Condition, operator_text, parse};
use crate::shell::word_pieces::{pieces, unquoted_literal};

use super::assignments::is_append;

const ORDERING_OPERATORS: &[&str] = &["<", ">", "<=", ">="];

// Las expansiones de una comparación con `<` o `>` y los valores de un `+=` que, según el flujo de
// datos, pueden ser un número (`numericalStatus` de ShellCheck).
#[derive(Debug, Default)]
pub struct NumericValues {
    starts: HashSet<usize>,
    integer_targets: HashSet<usize>,
}

impl NumericValues {
    pub fn of(context: &Context<'_, '_>) -> Self {
        let mut candidates = Vec::new();
        let mut appends = Vec::new();
        for node in descendants(context.root) {
            if node.kind() == TEST {
                ordering_operands(node, context.source, &mut candidates);
            } else if node.kind() == ASSIGNMENT && is_append(node, context.source) {
                candidates.extend(appended_value(node, context.source));
                appends.push(node);
            }
        }
        Self {
            starts: if candidates.is_empty() {
                HashSet::new()
            } else {
                numeric_expansions(context, &candidates)
            },
            integer_targets: if appends.is_empty() {
                HashSet::new()
            } else {
                integer_assignments(context, &appends)
            },
        }
    }

    pub fn contains_expansion_of(&self, word: Node<'_>) -> bool {
        sole_expansion(word).is_some_and(|expansion| self.starts.contains(&expansion.start_byte()))
    }

    // `variableMayBeDeclaredInteger`.
    pub fn assigns_to_integer(&self, assignment: Node<'_>) -> bool {
        self.integer_targets.contains(&assignment.start_byte())
    }

    pub fn contains_value(&self, value: Node<'_>) -> bool {
        self.starts.contains(&value.start_byte())
    }
}

fn ordering_operands<'tree>(test: Node<'tree>, source: &[u8], candidates: &mut Vec<Node<'tree>>) {
    let Some(condition) = parse(test, source) else {
        return;
    };
    condition.visit(true, &mut |part, _| {
        if let Condition::Binary(operator, left, right) = part
            && ORDERING_OPERATORS.contains(&operator_text(*operator, source))
        {
            candidates.extend([*left, *right].into_iter().filter_map(sole_expansion));
        }
    });
}

fn appended_value<'tree>(assignment: Node<'tree>, source: &[u8]) -> Option<Node<'tree>> {
    let value = assignment.child_by_field_name(VALUE_FIELD)?;
    let is_name = value.kind() == WORD && unquoted_literal(&pieces(value, source)).is_some();
    (is_name || EXPANSIONS.contains(&value.kind())).then_some(value)
}

fn sole_expansion(word: Node<'_>) -> Option<Node<'_>> {
    if EXPANSIONS.contains(&word.kind()) {
        return Some(word);
    }
    if word.kind() != STRING {
        return None;
    }
    let [only] = named_children(word)[..] else {
        return None;
    };
    EXPANSIONS.contains(&only.kind()).then_some(only)
}
