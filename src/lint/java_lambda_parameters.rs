use std::borrow::Cow;

use tree_sitter::Node;

use super::java::{NAME_FIELD, text};
use super::java_chains::argument_count;
use super::java_receivers::{UNKNOWN_TYPE_ARGUMENT, Variables, declared_arguments, inferred_parameters};

const INVOCATION: &str = "method_invocation";
const LAMBDA: &str = "lambda_expression";
const ARGUMENT_LIST: &str = "argument_list";
const IDENTIFIER: &str = "identifier";
const OBJECT_FIELD: &str = "object";
const FOR_EACH: &str = "forEach";
const MAP_TYPES: &[&str] = &[
    "Map",
    "HashMap",
    "LinkedHashMap",
    "TreeMap",
    "SortedMap",
    "NavigableMap",
    "ConcurrentMap",
    "ConcurrentHashMap",
    "EnumMap",
    "IdentityHashMap",
    "WeakHashMap",
    "Hashtable",
];
const ITERABLE_TYPES: &[&str] = &[
    "Iterable",
    "Collection",
    "List",
    "ArrayList",
    "LinkedList",
    "Vector",
    "CopyOnWriteArrayList",
    "Set",
    "HashSet",
    "LinkedHashSet",
    "TreeSet",
    "SortedSet",
    "NavigableSet",
    "Queue",
    "Deque",
    "ArrayDeque",
];
const MAP_ENTRY_PARTS: usize = 2;
const ELEMENT_PARTS: usize = 1;

// Las variables vistas desde una llamada cuyo receptor es un parámetro de la lambda de un
// `forEach` de colección o de mapa: ese parámetro tiene el tipo de los elementos, o de las claves y
// los valores, con que se declaró la colección.
pub fn with_lambda_parameters<'variables>(
    invocation: Node<'_>,
    source: &[u8],
    variables: &'variables Variables,
) -> Cow<'variables, Variables> {
    let receiver = chain_root(invocation).filter(|root| root.kind() == IDENTIFIER);
    let typed = receiver.and_then(|root| {
        let name = text(root, source);
        matches!(variables.get(name), Some(None))
            .then(|| lambda_parameter_type(root, name, source, variables))
            .flatten()
            .map(|declared| (name.to_owned(), declared))
    });
    match typed {
        Some((name, declared)) => {
            let mut scoped = variables.clone();
            scoped.insert(name, Some(declared));
            Cow::Owned(scoped)
        }
        None => Cow::Borrowed(variables),
    }
}

fn chain_root(invocation: Node<'_>) -> Option<Node<'_>> {
    let mut object = invocation.child_by_field_name(OBJECT_FIELD)?;
    while object.kind() == INVOCATION {
        object = object.child_by_field_name(OBJECT_FIELD)?;
    }
    Some(object)
}

fn lambda_parameter_type(
    usage: Node<'_>,
    name: &str,
    source: &[u8],
    variables: &Variables,
) -> Option<String> {
    let lambda = std::iter::successors(usage.parent(), Node::parent).find(|node| {
        node.kind() == LAMBDA
            && inferred_parameters(*node)
                .iter()
                .any(|parameter| text(*parameter, source) == name)
    })?;
    let parameters: Vec<&str> = inferred_parameters(lambda)
        .into_iter()
        .map(|parameter| text(parameter, source))
        .collect();
    let index = parameters.iter().position(|parameter| *parameter == name)?;
    let for_each = lambda
        .parent()
        .filter(|list| list.kind() == ARGUMENT_LIST)
        .and_then(|list| list.parent())
        .filter(|call| call.kind() == INVOCATION && argument_count(*call) == 1)
        .filter(|call| {
            call.child_by_field_name(NAME_FIELD)
                .is_some_and(|called| text(called, source) == FOR_EACH)
        })?;
    let collection = for_each
        .child_by_field_name(OBJECT_FIELD)
        .filter(|object| object.kind() == IDENTIFIER)?;
    let collection_name = text(collection, source);
    let Some(Some(declared)) = variables.get(collection_name) else {
        return None;
    };
    let expected = if MAP_TYPES.contains(&declared.as_str()) {
        MAP_ENTRY_PARTS
    } else if ITERABLE_TYPES.contains(&declared.as_str()) {
        ELEMENT_PARTS
    } else {
        return None;
    };
    let arguments = declared_arguments(collection, collection_name, source)?;
    (parameters.len() == expected && arguments.len() == expected)
        .then(|| arguments.into_iter().nth(index))
        .flatten()
        .filter(|argument| argument != UNKNOWN_TYPE_ARGUMENT)
}
