use tree_sitter::Node;

use super::qualified::is_typing;
use crate::lint::python::ast::{
    ASSIGNMENT, CLASS, CONCATENATED_STRING, FUNCTION, MODULE, STRING, TUPLE, named, text, unparenthesized,
};
use crate::rules::PythonRule;

// `as_pep_585_generic` de ruff: los genéricos de `typing` que ya tienen su versión en el builtin o en
// `collections`.
const PEP585_GENERICS: [&str; 11] = [
    "typing.Dict",
    "typing.FrozenSet",
    "typing.List",
    "typing.Set",
    "typing.Tuple",
    "typing.Type",
    "typing_extensions.Type",
    "typing.Deque",
    "typing_extensions.Deque",
    "typing.DefaultDict",
    "typing_extensions.DefaultDict",
];
const OPTIONAL: &str = "Optional";
const UNION: &str = "Union";
const NAMED_TUPLE: &str = "NamedTuple";
const STARRED: [&str; 2] = ["list_splat", "list_splat_pattern"];
const TYPE_NODE: &str = "type";
const IF: &str = "if_statement";
const CONDITION_FIELD: &str = "condition";
const CONSEQUENCE_FIELD: &str = "consequence";
const TYPE_CHECKING: &str = "TYPE_CHECKING";
const QUALIFIED_TYPE_CHECKING: &str = ".TYPE_CHECKING";
const ANNOTATED_HOLDERS: [&str; 3] = ["typed_parameter", "typed_default_parameter", FUNCTION];
const SCOPE_OWNERS: [&str; 3] = [FUNCTION, CLASS, MODULE];

// UP006: `List`, `Dict` y compañía de `typing`.
pub fn is_pep585_generic(qualified: &str) -> bool {
    PEP585_GENERICS.contains(&qualified)
}

// UP045 (`Optional[X]`) o UP007 (`Union[X, Y]`) para el valor de un subíndice. `slices` son los
// elementos de adentro, `qualify` da el nombre calificado de un nodo y `typing_context` dice si las
// anotaciones entre comillas se dejan sin evaluar (futuro, stubs).
pub fn pep604_rule(
    value: &str,
    slices: &[Node<'_>],
    qualify: &dyn Fn(Node<'_>) -> Option<String>,
    typing_context: bool,
) -> Option<PythonRule> {
    let rule = if is_typing(value, OPTIONAL) {
        PythonRule::UP045
    } else if is_typing(value, UNION) {
        PythonRule::UP007
    } else {
        return None;
    };
    let elements: Vec<Node<'_>> = slices.iter().flat_map(|slice| flattened(*slice)).collect();
    let quoted = elements
        .iter()
        .any(|element| matches!(element.kind(), STRING | CONCATENATED_STRING));
    let starred = elements.iter().any(|element| STARRED.contains(&element.kind()));
    let named_tuple = elements
        .iter()
        .any(|element| qualify(*element).is_some_and(|name| is_typing(&name, NAMED_TUPLE)));
    ((typing_context || !quoted) && !starred && !named_tuple).then_some(rule)
}

// Los elementos de un subíndice como los ve ruff: `X[a, b]` es una tupla de dos.
fn flattened(slice: Node<'_>) -> Vec<Node<'_>> {
    let inner = unwrap_type(unparenthesized(slice));
    if inner.kind() == TUPLE {
        named(inner)
            .into_iter()
            .map(|element| unwrap_type(unparenthesized(element)))
            .collect()
    } else {
        vec![inner]
    }
}

fn unwrap_type(node: Node<'_>) -> Node<'_> {
    if node.kind() == TYPE_NODE {
        named(node).first().copied().unwrap_or(node)
    } else {
        node
    }
}

// `execution_context().is_typing()` de ruff para un nodo fuera de una anotación entre comillas:
// adentro de `if TYPE_CHECKING:` o de una anotación que Python no evalúa (con el import del futuro,
// o la de una variable local o `self.x` dentro de una función).
pub fn is_typing_only(node: Node<'_>, future_annotations: bool, source: &[u8]) -> bool {
    let mut child = node;
    for ancestor in std::iter::successors(node.parent(), Node::parent) {
        match ancestor.kind() {
            IF if is_type_checking_body(ancestor, child, source) => return true,
            TYPE_NODE => {
                if let Some(typing) = ancestor
                    .parent()
                    .and_then(|holder| annotation_context(holder, future_annotations))
                {
                    return typing;
                }
            }
            _ => {}
        }
        child = ancestor;
    }
    false
}

// Si la anotación que cuelga de `holder` se evalúa en ejecución o no; `None` si no es una anotación.
fn annotation_context(holder: Node<'_>, future_annotations: bool) -> Option<bool> {
    if holder.kind() == ASSIGNMENT {
        return Some(future_annotations || is_inside_function(holder));
    }
    ANNOTATED_HOLDERS
        .contains(&holder.kind())
        .then_some(future_annotations)
}

fn is_type_checking_body(statement: Node<'_>, child: Node<'_>, source: &[u8]) -> bool {
    let in_body = statement
        .child_by_field_name(CONSEQUENCE_FIELD)
        .is_some_and(|body| body.id() == child.id());
    in_body
        && statement
            .child_by_field_name(CONDITION_FIELD)
            .is_some_and(|condition| {
                let name = text(condition, source);
                name == TYPE_CHECKING || name.ends_with(QUALIFIED_TYPE_CHECKING)
            })
}

fn is_inside_function(assignment: Node<'_>) -> bool {
    std::iter::successors(assignment.parent(), Node::parent)
        .find(|ancestor| SCOPE_OWNERS.contains(&ancestor.kind()))
        .is_some_and(|owner| owner.kind() == FUNCTION)
}
