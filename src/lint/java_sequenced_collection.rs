use tree_sitter::Node;

use super::java::{
    BODY_FIELD, IDENTIFIER, METHOD, NAME_FIELD, TYPE_FIELD, VARIABLE_DECLARATOR, is_type_declaration, text,
    unparenthesized,
};
use super::java_receivers::declared_type;
use super::{LintFinding, LintKind, finding};
use crate::fix::Fix;
use crate::rules::JavaRule;
use crate::syntax::{descendants, named_children};

const SEQUENCED_COLLECTIONS_RELEASE: u32 = 21;
const INVOCATION: &str = "method_invocation";
const FIELD_ACCESS: &str = "field_access";
const BINARY: &str = "binary_expression";
const FIELD_DECLARATION: &str = "field_declaration";
const BLOCK: &str = "block";
const THIS: &str = "this";
const THIS_QUALIFIER: &str = "this.";
const DECIMAL_LITERAL: &str = "decimal_integer_literal";
const OBJECT_FIELD: &str = "object";
const FIELD_FIELD: &str = "field";
const ARGUMENTS_FIELD: &str = "arguments";
const LEFT_FIELD: &str = "left";
const RIGHT_FIELD: &str = "right";
const OPERATOR_FIELD: &str = "operator";
const COMMENT_SUFFIX: &str = "comment";
const MINUS: &str = "-";
const ZERO: &str = "0";
const ONE: &str = "1";
const GET: &str = "get";
const REMOVE: &str = "remove";
const ADD: &str = "add";
const NEXT: &str = "next";
const ITERATOR: &str = "iterator";
const SIZE: &str = "size";
const FIRST_SUFFIX: &str = "First";
const LAST_SUFFIX: &str = "Last";
const GET_FIRST: &str = "getFirst";
const TYPE_ARGUMENTS_START: char = '<';
const NAME_SEPARATOR: char = '.';
const ADDED_ELEMENT: usize = 1;
const NEARBY_STATEMENTS: usize = 2;
const NEARBY_INDICES: std::ops::Range<u32> = 1..10;
const STOPS_THE_BLOCK_SEARCH: [&str; 7] = [
    "lambda_expression",
    "method_declaration",
    "constructor_declaration",
    "field_declaration",
    "class_body",
    "interface_body",
    "enum_body",
];
const LIST_TYPES: [&str; 9] = [
    "List",
    "ArrayList",
    "LinkedList",
    "Vector",
    "Stack",
    "CopyOnWriteArrayList",
    "AbstractList",
    "AbstractSequentialList",
    "ImmutableList",
];
const OTHER_SEQUENCED_TYPES: [&str; 15] = [
    "SequencedCollection",
    "SequencedSet",
    "Deque",
    "ArrayDeque",
    "BlockingDeque",
    "LinkedBlockingDeque",
    "ConcurrentLinkedDeque",
    "SortedSet",
    "NavigableSet",
    "TreeSet",
    "ConcurrentSkipListSet",
    "LinkedHashSet",
    "ImmutableSortedSet",
    "SortedMultiset",
    "ImmutableSortedMultiset",
];

// `SequencedCollectionMethodCanBeUsed` de IntelliJ: `get(0)`, `remove(0)`, `add(0, x)`,
// `get(size() - 1)` y `iterator().next()` donde Java 21 ya tiene `getFirst()`, `getLast()`,
// `removeFirst()`, `addFirst()`. El tipo del receptor sale de cómo se declara la variable.
pub fn reported(root: Node<'_>, source: &[u8], release: Option<u32>) -> Vec<LintFinding> {
    if release.is_some_and(|release| release < SEQUENCED_COLLECTIONS_RELEASE) {
        return Vec::new();
    }
    descendants(root)
        .into_iter()
        .filter(|node| node.kind() == INVOCATION)
        .filter_map(|call| {
            let name = call.child_by_field_name(NAME_FIELD)?;
            let (receiver, method) = replacement(call, text(name, source), source)?;
            let rest = if method.starts_with(ADD) {
                arguments(call)
                    .get(ADDED_ELEMENT)
                    .map_or_else(String::new, |element| text(*element, source).to_owned())
            } else {
                String::new()
            };
            let replacement = format!("{}.{method}({rest})", text(receiver, source));
            Some(finding(
                LintKind::Java(JavaRule::SequencedCollectionMethod),
                name,
                &method,
                Some(Fix::replace(call, &replacement, source)),
            ))
        })
        .collect()
}

fn replacement<'tree>(call: Node<'tree>, name: &str, source: &[u8]) -> Option<(Node<'tree>, String)> {
    let receiver = call.child_by_field_name(OBJECT_FIELD)?;
    let listed = arguments(call);
    match (name, listed.as_slice()) {
        (GET | REMOVE, [index]) => {
            let list = unparenthesized(receiver);
            if !is_of_type(list, &LIST_TYPES, source) {
                return None;
            }
            let index = unparenthesized(*index);
            if is_zero(index, source) && !has_different_index_nearby(call, name, list, source) {
                return Some((receiver, format!("{name}{FIRST_SUFFIX}")));
            }
            is_last_index(index, list, source).then(|| (receiver, format!("{name}{LAST_SUFFIX}")))
        }
        (ADD, [index, _]) => {
            let list = unparenthesized(receiver);
            let starts = is_of_type(list, &LIST_TYPES, source)
                && is_zero(unparenthesized(*index), source)
                && !has_different_index_nearby(call, name, list, source);
            starts.then(|| (receiver, format!("{ADD}{FIRST_SUFFIX}")))
        }
        (NEXT, []) => {
            let iterator = unparenthesized(receiver);
            let is_iterator_call = iterator.kind() == INVOCATION
                && iterator
                    .child_by_field_name(NAME_FIELD)
                    .is_some_and(|called| text(called, source) == ITERATOR)
                && arguments(iterator).is_empty();
            let collection = iterator.child_by_field_name(OBJECT_FIELD)?;
            let sequenced = is_iterator_call
                && (is_of_type(collection, &LIST_TYPES, source)
                    || is_of_type(collection, &OTHER_SEQUENCED_TYPES, source));
            sequenced.then(|| (collection, GET_FIRST.to_owned()))
        }
        _ => None,
    }
}

fn arguments(call: Node<'_>) -> Vec<Node<'_>> {
    call.child_by_field_name(ARGUMENTS_FIELD)
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .filter(|argument| !argument.kind().ends_with(COMMENT_SUFFIX))
        .collect()
}

fn is_zero(index: Node<'_>, source: &[u8]) -> bool {
    index.kind() == DECIMAL_LITERAL && text(index, source) == ZERO
}

fn is_last_index(index: Node<'_>, list: Node<'_>, source: &[u8]) -> bool {
    if index.kind() != BINARY
        || index
            .child_by_field_name(OPERATOR_FIELD)
            .is_none_or(|operator| text(operator, source) != MINUS)
    {
        return false;
    }
    let is_one = index
        .child_by_field_name(RIGHT_FIELD)
        .map(unparenthesized)
        .is_some_and(|right| right.kind() == DECIMAL_LITERAL && text(right, source) == ONE);
    let size_of_list = index
        .child_by_field_name(LEFT_FIELD)
        .map(unparenthesized)
        .is_some_and(|left| {
            left.kind() == INVOCATION
                && left
                    .child_by_field_name(NAME_FIELD)
                    .is_some_and(|called| text(called, source) == SIZE)
                && arguments(left).is_empty()
                && left
                    .child_by_field_name(OBJECT_FIELD)
                    .is_some_and(|sized| same_text(unparenthesized(sized), list, source))
        });
    is_one && size_of_list
}

fn is_of_type(receiver: Node<'_>, types: &[&str], source: &[u8]) -> bool {
    let Some(written) = receiver_type(receiver, source) else {
        return false;
    };
    let simple = simple_name(text(written, source));
    types.contains(&simple.as_str()) && !inside_type_named(receiver, &simple, source)
}

// El tipo escrito del receptor: una variable o un campo propio, el campo de una variable cuya clase
// se declara en el mismo archivo, o lo que devuelve un método de las clases que encierran la
// llamada (si todas sus declaraciones con ese nombre devuelven lo mismo).
fn receiver_type<'tree>(receiver: Node<'tree>, source: &[u8]) -> Option<Node<'tree>> {
    match receiver.kind() {
        IDENTIFIER => declared_type(receiver, text(receiver, source), source),
        FIELD_ACCESS => {
            let object = receiver.child_by_field_name(OBJECT_FIELD)?;
            let field = receiver.child_by_field_name(FIELD_FIELD)?;
            if object.kind() == THIS {
                return declared_type(field, text(field, source), source);
            }
            let owner = receiver_type(object, source)?;
            let owner_name = simple_name(text(owner, source));
            let declaration = type_declared_in_file(receiver, &owner_name, source)?;
            field_type(declaration, text(field, source), source)
        }
        INVOCATION if receiver.child_by_field_name(OBJECT_FIELD).is_none() => {
            let name = text(receiver.child_by_field_name(NAME_FIELD)?, source);
            own_method_type(receiver, name, source)
        }
        _ => None,
    }
}

fn simple_name(written: &str) -> String {
    let raw = written.split(TYPE_ARGUMENTS_START).next().unwrap_or_default();
    raw.rsplit(NAME_SEPARATOR).next().unwrap_or(raw).trim().to_owned()
}

fn type_declared_in_file<'tree>(usage: Node<'tree>, name: &str, source: &[u8]) -> Option<Node<'tree>> {
    let root = std::iter::successors(Some(usage), Node::parent).last()?;
    descendants(root).into_iter().find(|declaration| {
        is_type_declaration(*declaration)
            && declaration
                .child_by_field_name(NAME_FIELD)
                .is_some_and(|declared| text(declared, source) == name)
    })
}

fn field_type<'tree>(declaration: Node<'tree>, name: &str, source: &[u8]) -> Option<Node<'tree>> {
    let body = declaration.child_by_field_name(BODY_FIELD)?;
    named_children(body)
        .into_iter()
        .filter(|member| member.kind() == FIELD_DECLARATION)
        .find(|member| {
            named_children(*member).into_iter().any(|child| {
                child.kind() == VARIABLE_DECLARATOR
                    && child
                        .child_by_field_name(NAME_FIELD)
                        .is_some_and(|declared| text(declared, source) == name)
            })
        })
        .and_then(|member| member.child_by_field_name(TYPE_FIELD))
}

fn own_method_type<'tree>(call: Node<'tree>, name: &str, source: &[u8]) -> Option<Node<'tree>> {
    std::iter::successors(call.parent(), Node::parent)
        .filter(|ancestor| is_type_declaration(*ancestor))
        .find_map(|declaration| {
            let returned: Vec<Node<'tree>> = declaration
                .child_by_field_name(BODY_FIELD)
                .map(named_children)
                .unwrap_or_default()
                .into_iter()
                .filter(|member| {
                    member.kind() == METHOD
                        && member
                            .child_by_field_name(NAME_FIELD)
                            .is_some_and(|declared| text(declared, source) == name)
                })
                .filter_map(|method| method.child_by_field_name(TYPE_FIELD))
                .collect();
            let first = *returned.first()?;
            returned
                .iter()
                .all(|other| text(*other, source) == text(first, source))
                .then_some(first)
        })
}

fn inside_type_named(node: Node<'_>, name: &str, source: &[u8]) -> bool {
    std::iter::successors(node.parent(), Node::parent).any(|ancestor| {
        is_type_declaration(ancestor)
            && ancestor
                .child_by_field_name(NAME_FIELD)
                .is_some_and(|declared| text(declared, source) == name)
    })
}

// IntelliJ no avisa en una serie como `list.get(0); list.get(1);`: otra llamada al mismo método
// sobre la misma lista con un índice constante del 1 al 9, hasta dos sentencias antes o después.
fn has_different_index_nearby(call: Node<'_>, name: &str, list: Node<'_>, source: &[u8]) -> bool {
    let Some(block) = std::iter::successors(call.parent(), Node::parent)
        .take_while(|ancestor| !STOPS_THE_BLOCK_SEARCH.contains(&ancestor.kind()))
        .find(|ancestor| ancestor.kind() == BLOCK)
    else {
        return false;
    };
    let statements: Vec<Node<'_>> = named_children(block)
        .into_iter()
        .filter(|statement| !statement.kind().ends_with(COMMENT_SUFFIX))
        .collect();
    let Some(position) = statements.iter().position(|statement| {
        statement.start_byte() <= call.start_byte() && call.end_byte() <= statement.end_byte()
    }) else {
        return false;
    };
    let window_start = position.saturating_sub(NEARBY_STATEMENTS);
    let window_end = (position + NEARBY_STATEMENTS).min(statements.len().saturating_sub(1));
    statements
        .get(window_start..=window_end)
        .unwrap_or_default()
        .iter()
        .flat_map(|statement| descendants(*statement))
        .filter(|other| other.kind() == INVOCATION && other.id() != call.id())
        .any(|other| {
            other
                .child_by_field_name(NAME_FIELD)
                .is_some_and(|called| text(called, source) == name)
                && other
                    .child_by_field_name(OBJECT_FIELD)
                    .is_some_and(|object| same_text(unparenthesized(object), list, source))
                && arguments(other).first().is_some_and(|index| {
                    let index = unparenthesized(*index);
                    index.kind() == DECIMAL_LITERAL
                        && text(index, source)
                            .parse::<u32>()
                            .is_ok_and(|value| NEARBY_INDICES.contains(&value))
                })
        })
}

fn same_text(first: Node<'_>, second: Node<'_>, source: &[u8]) -> bool {
    let compact = |node: Node<'_>| -> String {
        let written: String = text(node, source)
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();
        written
            .strip_prefix(THIS_QUALIFIER)
            .map_or_else(|| written.clone(), str::to_owned)
    };
    compact(first) == compact(second)
}
