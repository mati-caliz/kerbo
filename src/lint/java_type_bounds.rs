use tree_sitter::Node;

use super::java::text;
use super::java_receivers::simple_type_name;
use crate::syntax::named_children;

const TYPE_PARAMETERS: &str = "type_parameters";
const TYPE_PARAMETER: &str = "type_parameter";
const TYPE_BOUND: &str = "type_bound";
const TYPE_IDENTIFIER: &str = "type_identifier";

// Si `written` es una variable de tipo de la clase o del método que encierran a `declaration`
// (`R extends AbstractBuild<P, R>`), el primer límite que declara: lo que se puede llamar sobre una
// variable de ese tipo sale del límite. Sin límite o si no es una variable de tipo, queda igual.
pub fn bounded_type(declaration: Node<'_>, written: String, source: &[u8]) -> String {
    let bound = std::iter::successors(declaration.parent(), Node::parent)
        .flat_map(named_children)
        .filter(|candidate| candidate.kind() == TYPE_PARAMETERS)
        .flat_map(named_children)
        .filter(|parameter| parameter.kind() == TYPE_PARAMETER)
        .find(|parameter| declares(*parameter, &written, source))
        .and_then(|parameter| first_bound(parameter, source));
    bound.unwrap_or(written)
}

fn declares(parameter: Node<'_>, name: &str, source: &[u8]) -> bool {
    named_children(parameter)
        .into_iter()
        .find(|child| child.kind() == TYPE_IDENTIFIER)
        .is_some_and(|identifier| text(identifier, source) == name)
}

fn first_bound(parameter: Node<'_>, source: &[u8]) -> Option<String> {
    let bound = named_children(parameter)
        .into_iter()
        .find(|child| child.kind() == TYPE_BOUND)?;
    named_children(bound)
        .first()
        .map(|first| simple_type_name(text(*first, source)))
}
