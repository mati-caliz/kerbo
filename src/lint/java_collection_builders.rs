use tree_sitter::Node;

use super::java::{IDENTIFIER, NAME_FIELD, TYPE_FIELD, text, unparenthesized};
use super::java_sequenced_receivers::simple_name;
use super::java_switch_assignment::local_declarator;
use crate::syntax::{descendants, named_children};

const INVOCATION: &str = "method_invocation";
const OBJECT_CREATION: &str = "object_creation_expression";
const EXPRESSION_STATEMENT: &str = "expression_statement";
const LOCAL_VARIABLE: &str = "local_variable_declaration";
const CLASS_BODY: &str = "class_body";
const BLOCK: &str = "block";
const THIS: &str = "this";
const OBJECT_FIELD: &str = "object";
const ARGUMENTS_FIELD: &str = "arguments";
const VALUE_FIELD: &str = "value";
const COMMENT_SUFFIX: &str = "comment";
const NUMERIC_LITERAL: &str = "decimal_integer_literal";
const MAP_ADDER: (&str, usize) = ("put", 2);
const COLLECTION_ADDER: (&str, usize) = ("add", 1);

// Los elementos de una colección armada a mano, como los junta `PrepopulatedCollectionModel` de
// IntelliJ: una variable local creada vacía (o con capacidad) seguida sólo de `add` (o `put`) hasta
// la sentencia que la usa, sin otros usos; o la doble llave `new ArrayList<>() {{ add(a); }}`.
pub(super) fn built_elements<'tree>(
    content: Node<'tree>,
    created: &str,
    is_map: bool,
    source: &[u8],
) -> Option<Vec<Node<'tree>>> {
    let adder = if is_map { MAP_ADDER } else { COLLECTION_ADDER };
    match content.kind() {
        IDENTIFIER => from_variable(content, created, adder, source),
        OBJECT_CREATION => from_double_brace(content, created, adder, source),
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

fn creates_empty(creation: Node<'_>, created: &str, source: &[u8]) -> bool {
    let anonymous = named_children(creation)
        .iter()
        .any(|child| child.kind() == CLASS_BODY);
    let capacity_only = match arguments(creation)[..] {
        [] => true,
        [capacity] => unparenthesized(capacity).kind() == NUMERIC_LITERAL,
        _ => false,
    };
    creation.kind() == OBJECT_CREATION
        && !anonymous
        && capacity_only
        && creation
            .child_by_field_name(TYPE_FIELD)
            .is_some_and(|written| simple_name(text(written, source)) == created)
}

// `x.add(a)` (o sin receptor, o con `this`, en la doble llave) con la cantidad de argumentos del
// método: los argumentos.
fn added<'tree>(
    statement: Node<'tree>,
    collection: Option<&str>,
    adder: (&str, usize),
    source: &[u8],
) -> Option<Vec<Node<'tree>>> {
    if statement.kind() != EXPRESSION_STATEMENT {
        return None;
    }
    let call = named_children(statement).into_iter().next()?;
    let named = call.kind() == INVOCATION
        && call
            .child_by_field_name(NAME_FIELD)
            .is_some_and(|name| text(name, source) == adder.0);
    let object = call.child_by_field_name(OBJECT_FIELD);
    let receives = collection.map_or_else(
        || object.is_none_or(|object| object.kind() == THIS),
        |variable| {
            object.is_some_and(|object| object.kind() == IDENTIFIER && text(object, source) == variable)
        },
    );
    let listed = arguments(call);
    (named && receives && listed.len() == adder.1).then_some(listed)
}

fn from_variable<'tree>(
    usage: Node<'tree>,
    created: &str,
    adder: (&str, usize),
    source: &[u8],
) -> Option<Vec<Node<'tree>>> {
    let name = text(usage, source);
    let declarator = local_declarator(usage, name, source)?;
    if !creates_empty(declarator.child_by_field_name(VALUE_FIELD)?, created, source) {
        return None;
    }
    let declaration = declarator
        .parent()
        .filter(|parent| parent.kind() == LOCAL_VARIABLE)?;
    let block = declaration.parent().filter(|parent| parent.kind() == BLOCK)?;
    let statements: Vec<Node<'tree>> = named_children(block)
        .into_iter()
        .filter(|statement| !statement.kind().ends_with(COMMENT_SUFFIX))
        .collect();
    let start = statements
        .iter()
        .position(|statement| statement.id() == declaration.id())?;
    let using = statements.iter().position(|statement| {
        statement.start_byte() <= usage.start_byte() && usage.end_byte() <= statement.end_byte()
    })?;
    let mut elements = Vec::new();
    for statement in statements.get(start + 1..using)? {
        elements.extend(added(*statement, Some(name), adder, source)?);
    }
    let references = descendants(block)
        .into_iter()
        .filter(|node| {
            node.kind() == IDENTIFIER
                && text(*node, source) == name
                && node.start_byte() > declarator.end_byte()
        })
        .count();
    let adds = using - start - 1;
    (references == adds + 1).then_some(elements)
}

fn from_double_brace<'tree>(
    creation: Node<'tree>,
    created: &str,
    adder: (&str, usize),
    source: &[u8],
) -> Option<Vec<Node<'tree>>> {
    let written = creation.child_by_field_name(TYPE_FIELD)?;
    if simple_name(text(written, source)) != created || !arguments(creation).is_empty() {
        return None;
    }
    let body = named_children(creation)
        .into_iter()
        .find(|child| child.kind() == CLASS_BODY)?;
    let [initializer] = named_children(body)
        .into_iter()
        .filter(|member| !member.kind().ends_with(COMMENT_SUFFIX))
        .collect::<Vec<_>>()[..]
    else {
        return None;
    };
    if initializer.kind() != BLOCK {
        return None;
    }
    let mut elements = Vec::new();
    for statement in named_children(initializer)
        .into_iter()
        .filter(|statement| !statement.kind().ends_with(COMMENT_SUFFIX))
    {
        elements.extend(added(statement, None, adder, source)?);
    }
    Some(elements)
}
