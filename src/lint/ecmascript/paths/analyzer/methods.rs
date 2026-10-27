use tree_sitter::Node;

use super::super::roles::children_with_fields;
use super::{Analyzer, BODY_FIELD, Children, DECORATOR, NAME_FIELD};

// Un método es dos nodos de ESTree: la clave va en el camino de afuera y la función en uno propio.
pub(super) fn visit_method<'tree>(
    analyzer: &mut Analyzer<'tree, '_>,
    method: Node<'tree>,
    field: Option<&'tree str>,
) {
    analyzer.preprocess(method, analyzer.place(field));
    analyzer.forward();
    analyzer.flow.at_enter.insert(method.id(), analyzer.reachable());
    let (key, function): (Children<'tree>, Children<'tree>) = children_with_fields(method)
        .into_iter()
        .partition(|(child, child_field)| child.kind() == DECORATOR || *child_field == Some(NAME_FIELD));
    analyzer.visit_children(method, field, key);
    if function
        .iter()
        .any(|(_, child_field)| *child_field == Some(BODY_FIELD))
    {
        analyzer.start_path(method);
        analyzer.forward();
        analyzer.visit_children(method, field, function);
        analyzer.forward();
        analyzer.flow.at_exit.insert(method.id(), analyzer.reachable());
        analyzer.end_path();
        analyzer.forward();
        return;
    }
    analyzer.visit_children(method, field, function);
    analyzer.forward();
    analyzer.flow.at_exit.insert(method.id(), analyzer.reachable());
}
