use tree_sitter::Node;

use super::java::{
    ARRAY_CREATION, CONSTRUCTOR, IDENTIFIER, METHOD, NAME_FIELD, OBJECT_CREATION, VARIABLE_DECLARATOR, text,
    unparenthesized,
};
use super::java_receivers::declared_type;
use super::{LintFinding, LintKind, finding};
use crate::rules::JavaRule;
use crate::syntax::{descendants, named_children};

const MAP_API_RELEASE: u32 = 8;
const IF: &str = "if_statement";
const BLOCK: &str = "block";
const EXPRESSION_STATEMENT: &str = "expression_statement";
const LOCAL_VARIABLE: &str = "local_variable_declaration";
const ASSIGNMENT: &str = "assignment_expression";
const UPDATE: &str = "update_expression";
const INVOCATION: &str = "method_invocation";
const BINARY: &str = "binary_expression";
const UNARY: &str = "unary_expression";
const NULL_LITERAL: &str = "null_literal";
const LAMBDA: &str = "lambda_expression";
const COMMENT_SUFFIX: &str = "comment";
const CONDITION_FIELD: &str = "condition";
const CONSEQUENCE_FIELD: &str = "consequence";
const ALTERNATIVE_FIELD: &str = "alternative";
const OBJECT_FIELD: &str = "object";
const ARGUMENTS_FIELD: &str = "arguments";
const LEFT_FIELD: &str = "left";
const RIGHT_FIELD: &str = "right";
const OPERATOR_FIELD: &str = "operator";
const OPERAND_FIELD: &str = "operand";
const VALUE_FIELD: &str = "value";
const ASSIGN: &str = "=";
const EQUALS: &str = "==";
const NOT_EQUALS: &str = "!=";
const NEGATION: &str = "!";
const GET: &str = "get";
const PUT: &str = "put";
const COMPUTE_IF_ABSENT: &str = "computeIfAbsent";
const TYPE_ARGUMENTS_START: char = '<';
const NAME_SEPARATOR: char = '.';
const UNSAFE_IN_CANDIDATE: [&str; 5] = [INVOCATION, OBJECT_CREATION, ASSIGNMENT, UPDATE, LAMBDA];
const MAP_TYPES: [&str; 20] = [
    "Map",
    "SortedMap",
    "NavigableMap",
    "SequencedMap",
    "HashMap",
    "LinkedHashMap",
    "TreeMap",
    "EnumMap",
    "IdentityHashMap",
    "WeakHashMap",
    "Hashtable",
    "ConcurrentMap",
    "ConcurrentNavigableMap",
    "ConcurrentHashMap",
    "ConcurrentSkipListMap",
    "AbstractMap",
    "BiMap",
    "HashBiMap",
    "ImmutableMap",
    "ImmutableBiMap",
];

struct MapAccess<'tree> {
    map: Node<'tree>,
    key: Node<'tree>,
    value: String,
}

// `Java8MapApi` de IntelliJ, la forma que pide `computeIfAbsent`: el valor se lee con `get`, y si es
// null se calcula, se asigna y se guarda con `put`. Sólo cuando lo que se calcula no puede tener
// efectos (un arreglo o un literal sin llamadas): con efectos, IntelliJ lo deja como información.
pub fn reported(root: Node<'_>, source: &[u8], release: Option<u32>) -> Vec<LintFinding> {
    if release.is_some_and(|release| release < MAP_API_RELEASE) {
        return Vec::new();
    }
    descendants(root)
        .into_iter()
        .filter(|node| node.kind() == IF && node.child_by_field_name(ALTERNATIVE_FIELD).is_none())
        .filter(|statement| computes_if_absent(*statement, source).is_some())
        .map(|statement| {
            finding(
                LintKind::Java(JavaRule::Java8MapApi),
                statement,
                COMPUTE_IF_ABSENT,
                None,
            )
        })
        .collect()
}

fn computes_if_absent(statement: Node<'_>, source: &[u8]) -> Option<()> {
    let value = null_checked_value(statement.child_by_field_name(CONDITION_FIELD)?, source)?;
    let access = map_read(previous_statement(statement)?, &value, source)?;
    let candidate = stored_candidate(statement.child_by_field_name(CONSEQUENCE_FIELD)?, &access, source)?;
    is_safe_candidate(candidate, &access.value, statement, source).then_some(())
}

// El nombre de la variable que la condición pregunta si es null (`v == null`, `!(v != null)`).
fn null_checked_value(condition: Node<'_>, source: &[u8]) -> Option<String> {
    let mut condition = unparenthesized(condition);
    let mut asks_for_null = false;
    while condition.kind() == UNARY
        && text(condition.child_by_field_name(OPERATOR_FIELD)?, source) == NEGATION
    {
        asks_for_null = !asks_for_null;
        condition = unparenthesized(condition.child_by_field_name(OPERAND_FIELD)?);
    }
    if condition.kind() != BINARY {
        return None;
    }
    match text(condition.child_by_field_name(OPERATOR_FIELD)?, source) {
        EQUALS => asks_for_null = !asks_for_null,
        NOT_EQUALS => {}
        _ => return None,
    }
    let left = unparenthesized(condition.child_by_field_name(LEFT_FIELD)?);
    let right = unparenthesized(condition.child_by_field_name(RIGHT_FIELD)?);
    let compared = match (left.kind(), right.kind()) {
        (_, NULL_LITERAL) => left,
        (NULL_LITERAL, _) => right,
        _ => return None,
    };
    (asks_for_null && compared.kind() == IDENTIFIER).then(|| text(compared, source).to_owned())
}

fn previous_statement(statement: Node<'_>) -> Option<Node<'_>> {
    std::iter::successors(statement.prev_named_sibling(), Node::prev_named_sibling)
        .find(|sibling| !sibling.kind().ends_with(COMMENT_SUFFIX))
}

// `V v = map.get(k);` o `v = map.get(k);` justo antes del `if`.
fn map_read<'tree>(previous: Node<'tree>, value: &str, source: &[u8]) -> Option<MapAccess<'tree>> {
    let read = match previous.kind() {
        LOCAL_VARIABLE => {
            let declarator = named_children(previous)
                .into_iter()
                .rfind(|child| child.kind() == VARIABLE_DECLARATOR)?;
            let declared = text(declarator.child_by_field_name(NAME_FIELD)?, source);
            (declared == value).then_some(declarator.child_by_field_name(VALUE_FIELD)?)?
        }
        EXPRESSION_STATEMENT => {
            let (target, assigned) = plain_assignment(named_children(previous).into_iter().next()?, source)?;
            (text(target, source) == value).then_some(assigned)?
        }
        _ => return None,
    };
    let (map, arguments) = map_call(read, GET, source)?;
    let [key] = arguments.as_slice() else {
        return None;
    };
    Some(MapAccess {
        map,
        key: *key,
        value: value.to_owned(),
    })
}

// Lo que se calcula en la rama del null: `map.put(k, v = calculo);` o `v = calculo; map.put(k, v);`.
fn stored_candidate<'tree>(
    branch: Node<'tree>,
    access: &MapAccess<'_>,
    source: &[u8],
) -> Option<Node<'tree>> {
    let statements = if branch.kind() == BLOCK {
        named_children(branch)
            .into_iter()
            .filter(|statement| !statement.kind().ends_with(COMMENT_SUFFIX))
            .collect()
    } else {
        vec![branch]
    };
    let (stored, assignment) = match statements.as_slice() {
        [put] => {
            let stored = stored_value(*put, access, source)?;
            (stored, unparenthesized(stored))
        }
        [assignment, put] => (
            stored_value(*put, access, source)?,
            named_children(*assignment).into_iter().next()?,
        ),
        _ => return None,
    };
    let (target, candidate) = plain_assignment(assignment, source)?;
    let stores_the_value = text(target, source) == access.value
        && (unparenthesized(stored).id() == assignment.id()
            || text(unparenthesized(stored), source) == access.value);
    stores_the_value.then_some(candidate)
}

fn stored_value<'tree>(statement: Node<'tree>, access: &MapAccess<'_>, source: &[u8]) -> Option<Node<'tree>> {
    if statement.kind() != EXPRESSION_STATEMENT {
        return None;
    }
    let (map, arguments) = map_call(named_children(statement).into_iter().next()?, PUT, source)?;
    let [key, stored] = arguments.as_slice() else {
        return None;
    };
    (equivalent(map, access.map, source) && equivalent(*key, access.key, source)).then_some(*stored)
}

fn map_call<'tree>(
    expression: Node<'tree>,
    method: &str,
    source: &[u8],
) -> Option<(Node<'tree>, Vec<Node<'tree>>)> {
    let call = unparenthesized(expression);
    let is_method =
        call.kind() == INVOCATION && text(call.child_by_field_name(NAME_FIELD)?, source) == method;
    let map = call.child_by_field_name(OBJECT_FIELD).filter(|_| is_method)?;
    if map.kind() != IDENTIFIER || !is_map(map, source) {
        return None;
    }
    Some((map, named_children(call.child_by_field_name(ARGUMENTS_FIELD)?)))
}

fn is_map(variable: Node<'_>, source: &[u8]) -> bool {
    declared_type(variable, text(variable, source), source).is_some_and(|written| {
        let raw = text(written, source)
            .split(TYPE_ARGUMENTS_START)
            .next()
            .unwrap_or_default();
        let simple = raw.rsplit(NAME_SEPARATOR).next().unwrap_or(raw).trim();
        MAP_TYPES.contains(&simple)
    })
}

fn plain_assignment<'tree>(expression: Node<'tree>, source: &[u8]) -> Option<(Node<'tree>, Node<'tree>)> {
    let is_plain = expression.kind() == ASSIGNMENT
        && text(expression.child_by_field_name(OPERATOR_FIELD)?, source) == ASSIGN;
    is_plain
        .then(|| {
            Some((
                expression.child_by_field_name(LEFT_FIELD)?,
                expression.child_by_field_name(RIGHT_FIELD)?,
            ))
        })
        .flatten()
}

// Lo calculado tiene que poder ir en una lambda sin efectos: un arreglo o literales, sin llamadas ni
// asignaciones, que no lea la propia variable y sólo lea variables que el método no reasigna.
fn is_safe_candidate(candidate: Node<'_>, value: &str, statement: Node<'_>, source: &[u8]) -> bool {
    let candidate = unparenthesized(candidate);
    let parts = descendants(candidate);
    let is_null = candidate.kind() == NULL_LITERAL;
    let has_unsafe = parts
        .iter()
        .any(|part| UNSAFE_IN_CANDIDATE.contains(&part.kind()));
    let builds_array_or_literal = candidate.kind() == ARRAY_CREATION || named_children(candidate).is_empty();
    if is_null || has_unsafe || !builds_array_or_literal {
        return false;
    }
    let Some(method) = std::iter::successors(statement.parent(), Node::parent)
        .find(|ancestor| [METHOD, CONSTRUCTOR].contains(&ancestor.kind()))
    else {
        return false;
    };
    let reassigned = reassigned_names(method, source);
    parts
        .iter()
        .filter(|part| part.kind() == IDENTIFIER)
        .map(|part| text(*part, source))
        .all(|name| name != value && !reassigned.contains(&name))
}

fn reassigned_names<'source>(method: Node<'_>, source: &'source [u8]) -> Vec<&'source str> {
    descendants(method)
        .into_iter()
        .filter_map(|node| match node.kind() {
            ASSIGNMENT => node.child_by_field_name(LEFT_FIELD),
            UPDATE => named_children(node).into_iter().next(),
            _ => None,
        })
        .filter(|target| target.kind() == IDENTIFIER)
        .map(|target| text(target, source))
        .collect()
}

fn equivalent(left: Node<'_>, right: Node<'_>, source: &[u8]) -> bool {
    let canonical =
        |node: Node<'_>| -> String { text(unparenthesized(node), source).split_whitespace().collect() };
    canonical(left) == canonical(right)
}
