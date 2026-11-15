use tree_sitter::Node;

use super::java::{NAME_FIELD, text, unparenthesized};
use super::java_sequenced_receivers::{inside_type_named, receiver_type, simple_name};
use super::{LintFinding, LintKind, finding};
use crate::fix::Fix;
use crate::rules::JavaRule;
use crate::syntax::{descendants, named_children, one_based};

const SEQUENCED_COLLECTIONS_RELEASE: u32 = 21;
const INVOCATION: &str = "method_invocation";
const BINARY: &str = "binary_expression";
const BLOCK: &str = "block";
const THIS_QUALIFIER: &str = "this.";
const DECIMAL_LITERAL: &str = "decimal_integer_literal";
const OBJECT_FIELD: &str = "object";
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
const ARRAY_TYPE: &str = "array_type";
const NAME_SEPARATOR: char = '.';
const TYPE_ARGUMENTS_START: char = '<';
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
    calls(root, source, release)
        .into_iter()
        .filter_map(|(call, name, found)| {
            if found.typed != Typed::Known {
                return None;
            }
            let rest = if found.method.starts_with(ADD) {
                arguments(call)
                    .get(ADDED_ELEMENT)
                    .map_or_else(String::new, |element| text(*element, source).to_owned())
            } else {
                String::new()
            };
            let replacement = format!("{}.{}({rest})", text(found.receiver, source), found.method);
            Some(finding(
                LintKind::Java(JavaRule::SequencedCollectionMethod),
                name,
                &found.method,
                Some(Fix::replace(call, &replacement, source)),
            ))
        })
        .collect()
}

// Los mismos casos cuando el receptor se declara con un tipo que no está en las tablas: el índice
// los resuelve con las dependencias (un `DomNodeList` de htmlunit es una `List`).
pub fn library_candidates(root: Node<'_>, source: &[u8], release: Option<u32>) -> Vec<SequencedCandidate> {
    calls(root, source, release)
        .into_iter()
        .filter_map(|(_, name, found)| {
            let Typed::Written(type_name) = found.typed else {
                return None;
            };
            let start = name.start_position();
            Some(SequencedCandidate {
                type_name,
                needs_list: found.needs_list,
                method: found.method,
                line: one_based(start.row),
                column: one_based(start.column),
            })
        })
        .collect()
}

fn calls<'tree>(
    root: Node<'tree>,
    source: &[u8],
    release: Option<u32>,
) -> Vec<(Node<'tree>, Node<'tree>, Detected<'tree>)> {
    if release.is_some_and(|release| release < SEQUENCED_COLLECTIONS_RELEASE) {
        return Vec::new();
    }
    descendants(root)
        .into_iter()
        .filter(|node| node.kind() == INVOCATION)
        .filter_map(|call| {
            let name = call.child_by_field_name(NAME_FIELD)?;
            let found = detected(call, text(name, source), source)?;
            Some((call, name, found))
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SequencedCandidate {
    pub type_name: String,
    pub needs_list: bool,
    pub method: String,
    pub line: usize,
    pub column: usize,
}

struct Detected<'tree> {
    receiver: Node<'tree>,
    method: String,
    typed: Typed,
    needs_list: bool,
}

fn detected<'tree>(call: Node<'tree>, name: &str, source: &[u8]) -> Option<Detected<'tree>> {
    let receiver = call.child_by_field_name(OBJECT_FIELD)?;
    let listed = arguments(call);
    let in_list = |method: String, typed: Typed| Detected {
        receiver,
        method,
        typed,
        needs_list: true,
    };
    match (name, listed.as_slice()) {
        (GET | REMOVE, [index]) => {
            let list = unparenthesized(receiver);
            let list_type = typed(list, true, source)?;
            let index = unparenthesized(*index);
            if is_zero(index, source) && !has_different_index_nearby(call, name, list, source) {
                return Some(in_list(format!("{name}{FIRST_SUFFIX}"), list_type));
            }
            is_last_index(index, list, source).then(|| in_list(format!("{name}{LAST_SUFFIX}"), list_type))
        }
        (ADD, [index, _]) => {
            let list = unparenthesized(receiver);
            let list_type = typed(list, true, source)?;
            let starts = is_zero(unparenthesized(*index), source)
                && !has_different_index_nearby(call, name, list, source);
            starts.then(|| in_list(format!("{ADD}{FIRST_SUFFIX}"), list_type))
        }
        (NEXT, []) => {
            let iterator = unparenthesized(receiver);
            let is_iterator_call = iterator.kind() == INVOCATION
                && iterator
                    .child_by_field_name(NAME_FIELD)
                    .is_some_and(|called| text(called, source) == ITERATOR)
                && arguments(iterator).is_empty();
            let collection = iterator.child_by_field_name(OBJECT_FIELD)?;
            let collection_type = typed(collection, false, source).filter(|_| is_iterator_call)?;
            Some(Detected {
                receiver: collection,
                method: GET_FIRST.to_owned(),
                typed: collection_type,
                needs_list: false,
            })
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

// Lo que se sabe del tipo del receptor: que está en las tablas, o el nombre escrito de otro tipo
// que hay que buscar en las dependencias para saber si es una lista o una colección secuenciada.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Typed {
    Known,
    Written(String),
}

fn typed(receiver: Node<'_>, needs_list: bool, source: &[u8]) -> Option<Typed> {
    let written = receiver_type(receiver, source)?;
    let simple = simple_name(text(written, source));
    if inside_type_named(receiver, &simple, source) {
        return None;
    }
    if is_sequenced_name(&simple, needs_list) {
        return Some(Typed::Known);
    }
    let raw = text(written, source)
        .split(TYPE_ARGUMENTS_START)
        .next()
        .unwrap_or_default()
        .trim();
    let is_class = raw.starts_with(|first: char| first.is_ascii_uppercase()) || raw.contains(NAME_SEPARATOR);
    (written.kind() != ARRAY_TYPE && is_class).then(|| Typed::Written(raw.to_owned()))
}

pub fn is_sequenced_name(simple: &str, needs_list: bool) -> bool {
    LIST_TYPES.contains(&simple) || !needs_list && OTHER_SEQUENCED_TYPES.contains(&simple)
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
