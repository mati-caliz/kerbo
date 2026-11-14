use tree_sitter::Node;

use super::java::{
    FINAL, IDENTIFIER, NAME_FIELD, VARIABLE_DECLARATOR, has_modifier, is_type_declaration, text,
};
use super::java_receivers::declared_type;
use super::{LintFinding, LintKind, finding};
use crate::fix::Fix;
use crate::rules::JavaRule;
use crate::syntax::{descendants, named_children};

const BINARY: &str = "binary_expression";
const INVOCATION: &str = "method_invocation";
const PARENTHESIZED: &str = "parenthesized_expression";
const DECIMAL_LITERAL: &str = "decimal_integer_literal";
const LEFT_FIELD: &str = "left";
const RIGHT_FIELD: &str = "right";
const OPERATOR_FIELD: &str = "operator";
const OBJECT_FIELD: &str = "object";
const ARGUMENTS_FIELD: &str = "arguments";
const VALUE_FIELD: &str = "value";
const SIZE_METHODS: [&str; 2] = ["size", "length"];
const NEGATION: &str = "!";
const IS_EMPTY_CALL: &str = ".isEmpty()";
const TYPE_ARGUMENTS_START: char = '<';
const NAME_SEPARATOR: char = '.';
const LONG_SUFFIXES: [char; 2] = ['L', 'l'];
const DIGIT_SEPARATOR: char = '_';
const DECIMAL_RADIX: u32 = 10;
const STRING_IS_EMPTY_RELEASE: u32 = 6;
const CHAR_SEQUENCE_IS_EMPTY_RELEASE: u32 = 15;
const LATER_IS_EMPTY: [(&str, u32); 4] = [
    ("String", STRING_IS_EMPTY_RELEASE),
    ("CharSequence", CHAR_SEQUENCE_IS_EMPTY_RELEASE),
    ("StringBuilder", CHAR_SEQUENCE_IS_EMPTY_RELEASE),
    ("StringBuffer", CHAR_SEQUENCE_IS_EMPTY_RELEASE),
];
const WITH_IS_EMPTY: [&str; 85] = [
    "Collection",
    "List",
    "Set",
    "Map",
    "Queue",
    "Deque",
    "SortedSet",
    "NavigableSet",
    "SortedMap",
    "NavigableMap",
    "SequencedCollection",
    "SequencedSet",
    "SequencedMap",
    "ArrayList",
    "LinkedList",
    "Vector",
    "Stack",
    "HashSet",
    "LinkedHashSet",
    "TreeSet",
    "EnumSet",
    "HashMap",
    "LinkedHashMap",
    "TreeMap",
    "EnumMap",
    "IdentityHashMap",
    "WeakHashMap",
    "Hashtable",
    "Properties",
    "ConcurrentHashMap",
    "ConcurrentMap",
    "ConcurrentNavigableMap",
    "ConcurrentSkipListMap",
    "ConcurrentSkipListSet",
    "CopyOnWriteArrayList",
    "CopyOnWriteArraySet",
    "ArrayDeque",
    "PriorityQueue",
    "BlockingQueue",
    "BlockingDeque",
    "LinkedBlockingQueue",
    "ArrayBlockingQueue",
    "LinkedBlockingDeque",
    "PriorityBlockingQueue",
    "DelayQueue",
    "AbstractCollection",
    "AbstractList",
    "AbstractSet",
    "AbstractMap",
    "AbstractQueue",
    "AbstractSequentialList",
    "Multimap",
    "ListMultimap",
    "SetMultimap",
    "SortedSetMultimap",
    "Multiset",
    "SortedMultiset",
    "BiMap",
    "Table",
    "RangeSet",
    "ImmutableCollection",
    "ImmutableList",
    "ImmutableSet",
    "ImmutableMap",
    "ImmutableSortedSet",
    "ImmutableSortedMap",
    "ImmutableMultiset",
    "ImmutableSortedMultiset",
    "ImmutableMultimap",
    "ImmutableListMultimap",
    "ImmutableSetMultimap",
    "ImmutableBiMap",
    "ImmutableTable",
    "ImmutableRangeSet",
    "HashMultimap",
    "ArrayListMultimap",
    "LinkedListMultimap",
    "LinkedHashMultimap",
    "TreeMultimap",
    "HashMultiset",
    "TreeMultiset",
    "LinkedHashMultiset",
    "EnumMultiset",
    "HashBiMap",
    "FluentIterable",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Comparison {
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}

impl Comparison {
    fn parse(operator: &str) -> Option<Self> {
        Some(match operator {
            "==" => Self::Equal,
            "!=" => Self::NotEqual,
            "<" => Self::Less,
            "<=" => Self::LessOrEqual,
            ">" => Self::Greater,
            ">=" => Self::GreaterOrEqual,
            _ => return None,
        })
    }

    const fn mirrored(self) -> Self {
        match self {
            Self::Less => Self::Greater,
            Self::LessOrEqual => Self::GreaterOrEqual,
            Self::Greater => Self::Less,
            Self::GreaterOrEqual => Self::LessOrEqual,
            Self::Equal => Self::Equal,
            Self::NotEqual => Self::NotEqual,
        }
    }

    const fn emptiness(self, compared: i64) -> Option<bool> {
        match (compared, self) {
            (0, Self::Equal) | (1, Self::Less) => Some(true),
            (0, Self::NotEqual | Self::Greater) | (1, Self::GreaterOrEqual) => Some(false),
            _ => None,
        }
    }
}

pub fn reported(root: Node<'_>, source: &[u8], release: Option<u32>) -> Vec<LintFinding> {
    descendants(root)
        .into_iter()
        .filter(|node| node.kind() == BINARY)
        .filter_map(|comparison| {
            let replacement = replacement(comparison, source, release)?;
            Some(finding(
                LintKind::Java(JavaRule::SizeReplaceableByIsEmpty),
                comparison,
                &replacement,
                Some(Fix::replace(comparison, &replacement, source)),
            ))
        })
        .collect()
}

fn replacement(comparison: Node<'_>, source: &[u8], release: Option<u32>) -> Option<String> {
    let operator = Comparison::parse(text(comparison.child_by_field_name(OPERATOR_FIELD)?, source))?;
    let left = comparison.child_by_field_name(LEFT_FIELD)?;
    let right = comparison.child_by_field_name(RIGHT_FIELD)?;
    let (call, other, operator) = if is_size_call(left, source) {
        (left, right, operator)
    } else {
        (right, left, operator.mirrored())
    };
    if !is_size_call(call, source) {
        return None;
    }
    let receiver = call.child_by_field_name(OBJECT_FIELD)?;
    if receiver.kind() != IDENTIFIER || !has_is_empty(receiver, source, release) {
        return None;
    }
    let empty = operator.emptiness(integer_constant(other, source)?)?;
    let prefix = if empty { "" } else { NEGATION };
    Some(format!("{prefix}{}{IS_EMPTY_CALL}", text(receiver, source)))
}

fn is_size_call(node: Node<'_>, source: &[u8]) -> bool {
    node.kind() == INVOCATION
        && node
            .child_by_field_name(NAME_FIELD)
            .is_some_and(|name| SIZE_METHODS.contains(&text(name, source)))
        && node
            .child_by_field_name(ARGUMENTS_FIELD)
            .is_some_and(|arguments| named_children(arguments).is_empty())
}

fn has_is_empty(receiver: Node<'_>, source: &[u8], release: Option<u32>) -> bool {
    let Some(written) = declared_type(receiver, text(receiver, source), source) else {
        return false;
    };
    let raw = text(written, source)
        .split(TYPE_ARGUMENTS_START)
        .next()
        .unwrap_or_default();
    let simple = raw.rsplit(NAME_SEPARATOR).next().unwrap_or(raw).trim();
    let known = WITH_IS_EMPTY.contains(&simple)
        || LATER_IS_EMPTY
            .iter()
            .any(|&(name, since)| name == simple && release.is_none_or(|release| release >= since));
    known && !inside_type_named(receiver, simple, source)
}

fn inside_type_named(node: Node<'_>, name: &str, source: &[u8]) -> bool {
    std::iter::successors(node.parent(), Node::parent).any(|ancestor| {
        is_type_declaration(ancestor)
            && ancestor
                .child_by_field_name(NAME_FIELD)
                .is_some_and(|declared| text(declared, source) == name)
    })
}

fn integer_constant(node: Node<'_>, source: &[u8]) -> Option<i64> {
    match node.kind() {
        DECIMAL_LITERAL => parse_integer(text(node, source)),
        PARENTHESIZED => integer_constant(named_children(node).into_iter().next()?, source),
        IDENTIFIER => final_initializer(node, source).and_then(|value| integer_constant(value, source)),
        _ => None,
    }
}

fn parse_integer(literal: &str) -> Option<i64> {
    let digits: String = literal
        .trim_end_matches(LONG_SUFFIXES)
        .chars()
        .filter(|character| *character != DIGIT_SEPARATOR)
        .collect();
    i64::from_str_radix(&digits, DECIMAL_RADIX).ok()
}

fn final_initializer<'tree>(usage: Node<'tree>, source: &[u8]) -> Option<Node<'tree>> {
    let name = text(usage, source);
    std::iter::successors(usage.parent(), Node::parent).find_map(|scope| {
        named_children(scope).into_iter().find_map(|statement| {
            let declarator = named_children(statement).into_iter().find(|child| {
                child.kind() == VARIABLE_DECLARATOR
                    && child
                        .child_by_field_name(NAME_FIELD)
                        .is_some_and(|declared| text(declared, source) == name)
            })?;
            has_modifier(statement, FINAL)
                .then(|| declarator.child_by_field_name(VALUE_FIELD))
                .flatten()
        })
    })
}
