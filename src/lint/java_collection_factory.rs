use tree_sitter::Node;

use super::java::{IDENTIFIER, NAME_FIELD, TYPE_FIELD, text, unparenthesized};
use super::java_sequenced_receivers::{receiver_type, simple_name};
use super::{LintFinding, LintKind, finding};
use crate::rules::JavaRule;
use crate::syntax::{descendants, named_children};

const COLLECTION_FACTORIES_RELEASE: u32 = 9;
const COPY_FACTORIES_RELEASE: u32 = 10;
const INVOCATION: &str = "method_invocation";
const OBJECT_CREATION: &str = "object_creation_expression";
const FIELD_ACCESS: &str = "field_access";
const CLASS_BODY: &str = "class_body";
const CLASS_LITERAL: &str = "class_literal";
const STRING_LITERAL: &str = "string_literal";
const BINARY: &str = "binary_expression";
const UNARY: &str = "unary_expression";
const OBJECT_FIELD: &str = "object";
const ARGUMENTS_FIELD: &str = "arguments";
const COMMENT_SUFFIX: &str = "comment";
const COLLECTIONS: [&str; 2] = ["Collections", "java.util.Collections"];
const ARRAYS: [&str; 2] = ["Arrays", "java.util.Arrays"];
const OBJECTS: [&str; 2] = ["Objects", "java.util.Objects"];
const AS_LIST: &str = "asList";
const REQUIRE_NON_NULL: &str = "requireNonNull";
const MAP_VIEWS: [&str; 3] = ["values", "keySet", "entrySet"];
const MAP_COPY_SOURCES: [&str; 3] = ["Map", "AbstractMap", "HashMap"];
const NUMERIC_TYPES: [&str; 8] = ["int", "long", "short", "byte", "Integer", "Long", "Short", "Byte"];
const COUNTING_METHODS: [&str; 3] = ["size", "length", "count"];
const CONSTANT_LITERALS: [&str; 7] = [
    STRING_LITERAL,
    "character_literal",
    "decimal_integer_literal",
    "hex_integer_literal",
    "decimal_floating_point_literal",
    "true",
    "false",
];
const LIST: &str = "List";
const MAP: &str = "Map";
const NULLABLE_MARK: &str = "@Nullable";

#[derive(Clone, Copy)]
struct Kind {
    wrapper: &'static str,
    created: &'static str,
    factory: &'static str,
}

const KINDS: [Kind; 3] = [
    Kind {
        wrapper: "unmodifiableList",
        created: "ArrayList",
        factory: LIST,
    },
    Kind {
        wrapper: "unmodifiableSet",
        created: "HashSet",
        factory: "Set",
    },
    Kind {
        wrapper: "unmodifiableMap",
        created: "HashMap",
        factory: MAP,
    },
];

// `Java9CollectionFactory` de IntelliJ en las formas que no piden seguir una variable:
// `Collections.unmodifiableList(new ArrayList<>(otra))` es `List.copyOf(otra)` desde Java 10 si
// `otra` es una colección, y `Collections.unmodifiableList(Arrays.asList(a, b))` es `List.of(a, b)`
// si ningún elemento puede ser null (y en un `Set`, ninguno se repite). Igual con `Set` y `Map`.
pub fn reported(root: Node<'_>, source: &[u8], release: Option<u32>) -> Vec<LintFinding> {
    if release.is_some_and(|release| release < COLLECTION_FACTORIES_RELEASE) {
        return Vec::new();
    }
    let copies = release.is_none_or(|release| release >= COPY_FACTORIES_RELEASE);
    descendants(root)
        .into_iter()
        .filter(|node| node.kind() == INVOCATION)
        .filter_map(|call| {
            let name = call.child_by_field_name(NAME_FIELD)?;
            let kind = KINDS.iter().find(|kind| kind.wrapper == text(name, source))?;
            let receiver = call.child_by_field_name(OBJECT_FIELD)?;
            let [content] = arguments(call)[..] else {
                return None;
            };
            if !COLLECTIONS.contains(&text(receiver, source)) {
                return None;
            }
            let factory = replacement(unparenthesized(content), *kind, copies, source)?;
            Some(finding(
                LintKind::Java(JavaRule::Java9CollectionFactory),
                name,
                &format!("{}.{factory}", kind.factory),
                None,
            ))
        })
        .collect()
}

fn arguments(call: Node<'_>) -> Vec<Node<'_>> {
    call.child_by_field_name(ARGUMENTS_FIELD)
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .filter(|argument| !argument.kind().ends_with(COMMENT_SUFFIX))
        .collect()
}

fn replacement(content: Node<'_>, kind: Kind, allows_copy_of: bool, source: &[u8]) -> Option<&'static str> {
    match content.kind() {
        INVOCATION if kind.factory != MAP => fixed_elements(content, kind, source),
        OBJECT_CREATION => {
            let created = content.child_by_field_name(TYPE_FIELD)?;
            let anonymous = named_children(content)
                .iter()
                .any(|child| child.kind() == CLASS_BODY);
            if anonymous || simple_name(text(created, source)) != kind.created {
                return None;
            }
            let [copied] = arguments(content)[..] else {
                return None;
            };
            let copied = unparenthesized(copied);
            if kind.factory != MAP && is_as_list(copied, source) {
                return fixed_elements(copied, kind, source);
            }
            (allows_copy_of && is_copyable(copied, kind, source)).then_some("copyOf")
        }
        _ => None,
    }
}

fn is_as_list(call: Node<'_>, source: &[u8]) -> bool {
    call.kind() == INVOCATION
        && call
            .child_by_field_name(NAME_FIELD)
            .is_some_and(|name| text(name, source) == AS_LIST)
        && call
            .child_by_field_name(OBJECT_FIELD)
            .is_some_and(|receiver| ARRAYS.contains(&text(receiver, source)))
}

fn fixed_elements(call: Node<'_>, kind: Kind, source: &[u8]) -> Option<&'static str> {
    if !is_as_list(call, source) || kind.factory == MAP {
        return None;
    }
    let elements = arguments(call);
    let all_present = elements.iter().all(|element| is_non_null(*element, source));
    let mut constants: Vec<&str> = elements
        .iter()
        .filter(|element| CONSTANT_LITERALS.contains(&element.kind()))
        .map(|element| text(*element, source))
        .collect();
    let total = constants.len();
    constants.sort_unstable();
    constants.dedup();
    let repeats = kind.factory != LIST && constants.len() != total;
    (all_present && !repeats).then_some("of")
}

fn is_non_null(element: Node<'_>, source: &[u8]) -> bool {
    let element = unparenthesized(element);
    match element.kind() {
        kind if CONSTANT_LITERALS.contains(&kind) || kind == CLASS_LITERAL || kind == OBJECT_CREATION => true,
        BINARY => descendants(element)
            .iter()
            .any(|part| part.kind() == STRING_LITERAL),
        INVOCATION => {
            element
                .child_by_field_name(NAME_FIELD)
                .is_some_and(|name| text(name, source) == REQUIRE_NON_NULL)
                && element
                    .child_by_field_name(OBJECT_FIELD)
                    .is_some_and(|receiver| OBJECTS.contains(&text(receiver, source)))
        }
        _ => false,
    }
}

// Lo que se copia en un `ArrayList` o un `HashSet` sólo compila si es una colección o un int (la
// capacidad), así que alcanza con que no sea un número y no tenga `@Nullable` en sus elementos. Para
// `Map.copyOf`, IntelliJ pide que el tipo de origen sea un supertipo de `HashMap`.
fn is_copyable(copied: Node<'_>, kind: Kind, source: &[u8]) -> bool {
    let written = declared(copied, source).map(|written| text(written, source));
    if written.is_some_and(|written| written.contains(NULLABLE_MARK)) {
        return false;
    }
    if kind.factory == MAP {
        return written.is_some_and(|written| MAP_COPY_SOURCES.contains(&simple_name(written).as_str()));
    }
    is_map_view(copied, source) || !is_numeric(copied, written, source)
}

fn is_numeric(copied: Node<'_>, written: Option<&str>, source: &[u8]) -> bool {
    match copied.kind() {
        BINARY | UNARY => true,
        kind if CONSTANT_LITERALS.contains(&kind) => true,
        INVOCATION => copied
            .child_by_field_name(NAME_FIELD)
            .is_some_and(|name| COUNTING_METHODS.contains(&text(name, source))),
        _ => written.is_none_or(|written| NUMERIC_TYPES.contains(&written)),
    }
}

fn declared<'tree>(expression: Node<'tree>, source: &[u8]) -> Option<Node<'tree>> {
    match expression.kind() {
        IDENTIFIER | FIELD_ACCESS | INVOCATION => receiver_type(expression, source),
        _ => None,
    }
}

fn is_map_view(call: Node<'_>, source: &[u8]) -> bool {
    call.kind() == INVOCATION
        && call
            .child_by_field_name(NAME_FIELD)
            .is_some_and(|name| MAP_VIEWS.contains(&text(name, source)))
        && arguments(call).is_empty()
}
