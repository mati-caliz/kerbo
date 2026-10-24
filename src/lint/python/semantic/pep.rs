use tree_sitter::Node;

use super::qualified::is_typing;
use crate::lint::python::ast::{
    ASSIGNMENT, ATTRIBUTE, CLASS, CONCATENATED_STRING, FUNCTION, IDENTIFIER, MODULE, STRING, SUBSCRIPT,
    SUBSCRIPT_FIELD, TUPLE, fields, named, text, unparenthesized,
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
const UNPACK: &str = "Unpack";
const UNPACKABLE: [&str; 5] = [IDENTIFIER, ATTRIBUTE, SUBSCRIPT, "generic_type", "member_type"];
const TYPE_PARAMETER: &str = "type_parameter";
const TYPED_PARAMETER: &str = "typed_parameter";
const LIST_SPLAT_PATTERN: &str = "list_splat_pattern";
const TYPE_FIELD: &str = "type";

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

// UP044: `Unpack[X]` donde Python 3.11 acepta `*X`, adentro de un subíndice o como tipo de `*args`.
pub fn is_pep646_unpack(value: &str, subscript: Node<'_>, slices: &[Node<'_>]) -> bool {
    let [slice] = slices else {
        return false;
    };
    is_typing(value, UNPACK) && UNPACKABLE.contains(&unwrap_type(*slice).kind()) && accepts_star(subscript)
}

fn accepts_star(node: Node<'_>) -> bool {
    let mut child = node;
    let mut parent = node.parent();
    while let Some(wrapper) = parent.filter(|wrapper| wrapper.kind() == TYPE_NODE) {
        child = wrapper;
        parent = wrapper.parent();
    }
    let Some(parent) = parent else {
        return false;
    };
    match parent.kind() {
        TYPE_PARAMETER => true,
        SUBSCRIPT => is_subscript_index(parent, child),
        TUPLE => parent
            .parent()
            .is_some_and(|subscript| subscript.kind() == SUBSCRIPT && is_subscript_index(subscript, parent)),
        TYPED_PARAMETER => {
            named(parent)
                .first()
                .is_some_and(|target| target.kind() == LIST_SPLAT_PATTERN)
                && parent
                    .child_by_field_name(TYPE_FIELD)
                    .is_some_and(|annotation| annotation.id() == child.id())
        }
        _ => false,
    }
}

fn is_subscript_index(subscript: Node<'_>, child: Node<'_>) -> bool {
    fields(subscript, SUBSCRIPT_FIELD)
        .iter()
        .any(|index| index.id() == child.id())
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

// `in_typing_only_annotation` de ruff: adentro de una anotación que Python no evalúa. A diferencia de
// `is_typing_only`, un `if TYPE_CHECKING:` no cuenta.
pub fn is_typing_only_annotation(node: Node<'_>, deferred_annotations: bool) -> bool {
    std::iter::successors(node.parent(), Node::parent)
        .filter(|ancestor| ancestor.kind() == TYPE_NODE)
        .find_map(|ancestor| {
            ancestor
                .parent()
                .and_then(|holder| annotation_context(holder, deferred_annotations))
        })
        .unwrap_or(false)
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
