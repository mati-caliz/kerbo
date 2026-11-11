use tree_sitter::Node;

use super::Scope;
use crate::deploy::django::constants::Constants;
use crate::deploy::django::operations::{Located, Operation};
use crate::deploy::python_calls::Call;
use crate::syntax::named_children;

const LIST_COMPREHENSION: &str = "list_comprehension";
const FOR_IN_CLAUSE: &str = "for_in_clause";
const IDENTIFIER: &str = "identifier";

pub(super) type ReadOperation = dyn for<'bound> Fn(Scope<'bound>, &Call<'bound>) -> Option<Operation>;

pub(super) fn expanded_comprehension<'context>(
    scope: Scope<'context>,
    node: Node<'context>,
    read: &ReadOperation,
) -> Option<Vec<Located>> {
    if node.kind() != LIST_COMPREHENSION {
        return None;
    }
    let body = node.child_by_field_name("body")?;
    let clauses: Vec<Node<'_>> = named_children(node)
        .into_iter()
        .filter(|child| child.id() != body.id())
        .collect();
    let [clause] = clauses.as_slice() else {
        return None;
    };
    let variable = clause
        .child_by_field_name("left")
        .filter(|variable| clause.kind() == FOR_IN_CLAUSE && variable.kind() == IDENTIFIER)?;
    let name = variable.utf8_text(scope.source).ok()?;
    let iterable = clause.child_by_field_name("right")?;
    Some(
        scope
            .sequence(iterable)
            .into_iter()
            .filter_map(|element| {
                let mut bindings = scope.constants.bindings.to_vec();
                bindings.push((name.to_owned(), scope.resolved(element)));
                let bound = Scope {
                    constants: Constants::new(scope.source, &bindings),
                    ..scope
                };
                bound.located_call(body, read)
            })
            .collect(),
    )
}
