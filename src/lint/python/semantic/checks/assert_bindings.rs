use tree_sitter::Node;

use super::super::Visitor;
use super::super::model::{BindingId, BindingKind};
use super::type_checking_imports::node_at;
use crate::lint::python::Report;
use crate::rules::PythonRule;

const ASSERT: &str = "assert_statement";
const NAMED_EXPRESSION: &str = "named_expression";

// RUF018: `assert (x := valor)` con `x` usado fuera de un `assert`, que desaparece con `python -O`.
// Avisa en la expresión con `:=`.
pub fn assignments_in_assert(visitor: &Visitor<'_>, latest: &[(&str, BindingId)]) -> Vec<Report> {
    latest
        .iter()
        .filter_map(|(_, id)| visitor.model.binding(*id))
        .filter(|binding| binding.kind == BindingKind::NamedExpression)
        .filter_map(|binding| {
            let name = binding.site.node?;
            let expression = name.parent().filter(|parent| parent.kind() == NAMED_EXPRESSION)?;
            if !in_assert(name) {
                return None;
            }
            let root = std::iter::successors(Some(name), Node::parent).last()?;
            let used_outside = binding
                .references
                .iter()
                .any(|reference| node_at(root, reference.at).is_some_and(|node| !in_assert(node)));
            used_outside.then(|| Report::at(PythonRule::RUF018, expression, ""))
        })
        .collect()
}

fn in_assert(node: Node<'_>) -> bool {
    std::iter::successors(Some(node), Node::parent).any(|ancestor| ancestor.kind() == ASSERT)
}
