mod collapse_fix;
mod needless_bool;

use tree_sitter::Node;

use super::Report;
use super::ast::{
    ARGUMENTS_FIELD, ATTRIBUTE, ATTRIBUTE_FIELD, BLOCK, CALL, COMMENT, COMPARISON, FALSE, FUNCTION_FIELD,
    IDENTIFIER, LEFT_FIELD, OBJECT_FIELD, RIGHT_FIELD, STRING, TRUE, named, text, unparenthesized,
};
use super::escapes::text_value;
use super::known_types::{Known, is_known};
use crate::fix::Fix;
use crate::rules::PythonRule;
use collapse_fix::{collapse_fix, merge_with_fix};
use needless_bool::needless_bool;

const IF: &str = "if_statement";
const ELSE: &str = "else_clause";
const WITH: &str = "with_statement";
const FOR: &str = "for_statement";
const FOR_IN: &str = "for_in_clause";
const KEYS: &str = "keys";
const DOT: &str = ".";
const SELF: &str = "self";
const MEMBERSHIP_OPERATORS: [&[&str]; 2] = [&["in"], &["not", "in"]];
const WITH_CLAUSE: &str = "with_clause";
const WITH_ITEM: &str = "with_item";
const ASYNC: &str = "async";
const CONDITION_FIELD: &str = "condition";
const CONSEQUENCE_FIELD: &str = "consequence";
const ALTERNATIVE_FIELD: &str = "alternative";
const BODY_FIELD: &str = "body";
const VALUE_FIELD: &str = "value";
const MAIN_NAME: &str = "__name__";
const MAIN_VALUE: &str = "__main__";
const TYPE_CHECKING: &str = "TYPE_CHECKING";
const SYS_MODULE: &str = "sys";
const SYSTEM_MEMBERS: [&str; 2] = ["version_info", "platform"];
const IMPORT: &str = "import_statement";
const IMPORT_FROM: &str = "import_from_statement";
const ALIASED_IMPORT: &str = "aliased_import";
const NAME_FIELD: &str = "name";
const ALIAS_FIELD: &str = "alias";
const MODULE_NAME_FIELD: &str = "module_name";
const TIMEOUT_SCOPES: [&str; 9] = [
    "asyncio.timeout",
    "asyncio.timeout_at",
    "anyio.CancelScope",
    "anyio.fail_after",
    "anyio.move_on_after",
    "trio.fail_after",
    "trio.fail_at",
    "trio.move_on_after",
    "trio.move_on_at",
];

// Cómo se llaman `sys.version_info` y `sys.platform` en el archivo: con el módulo (con su alias) o
// importados sueltos.
struct SystemNames {
    modules: Vec<String>,
    members: Vec<String>,
}

pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    let system = system_names(nodes, source);
    let is_excluded = |condition: Node<'_>| {
        mentions_system(condition, source, &system) || is_type_checking(condition, source)
    };
    let mut found: Vec<Report> = nodes
        .iter()
        .filter(|node| node.kind() == IF)
        .filter_map(|statement| needless_bool(*statement, &is_excluded, source))
        .collect();
    found.extend(nodes.iter().filter_map(|node| match node.kind() {
        IF => collapsible_if(*node, source, &system),
        WITH => multiple_with(*node, source),
        FOR | FOR_IN => {
            let target = node.child_by_field_name(LEFT_FIELD)?;
            in_dict_keys(target, node.child_by_field_name(RIGHT_FIELD)?, source)
        }
        COMPARISON => match named(*node).as_slice() {
            [left, right] if is_membership(*left, *right, source) => in_dict_keys(*left, *right, source),
            _ => None,
        },
        _ => None,
    }));
    found
}

fn statements(block: Option<Node<'_>>) -> Vec<Node<'_>> {
    block.map_or_else(Vec::new, |block| {
        named(block)
            .into_iter()
            .filter(|child| child.kind() != COMMENT)
            .collect()
    })
}

// SIM102: un `if` (o el último `elif`) cuyo único contenido es otro `if` sin ramas, que se pueden
// juntar con `and`.
fn collapsible_if(statement: Node<'_>, source: &[u8], system: &SystemNames) -> Option<Report> {
    let candidate = nested_if_candidate(statement, source)?;
    find_last_nested_if(&statements(
        candidate
            .child_by_field_name(CONSEQUENCE_FIELD)
            .or_else(|| body_of(candidate)),
    ))?;
    let condition = statement.child_by_field_name(CONDITION_FIELD)?;
    let skipped = mentions_system(condition, source, system) || is_type_checking(condition, source);
    (!skipped)
        .then(|| Report::at(PythonRule::SIM102, candidate, "").with_fix(collapse_fix(candidate, source)))
}

// `nested_if_body`: el `if` o su último `elif`, si se puede juntar con lo que tiene adentro.
fn nested_if_candidate<'tree>(statement: Node<'tree>, source: &[u8]) -> Option<Node<'tree>> {
    let clauses: Vec<Node<'tree>> = {
        let mut cursor = statement.walk();
        statement
            .children_by_field_name(ALTERNATIVE_FIELD, &mut cursor)
            .collect()
    };
    let (candidate, test) = match clauses.last() {
        Some(clause) if clause.kind() == ELSE => return None,
        Some(clause) => (*clause, clause.child_by_field_name(CONDITION_FIELD)?),
        None => (statement, statement.child_by_field_name(CONDITION_FIELD)?),
    };
    let body_length = statements(statement.child_by_field_name(CONSEQUENCE_FIELD)).len();
    let test = unparenthesized(test);
    let unmergeable = body_length > 1 || is_main_check(test, source) || [TRUE, FALSE].contains(&test.kind());
    (!unmergeable).then_some(candidate)
}

fn body_of(clause: Node<'_>) -> Option<Node<'_>> {
    clause
        .child_by_field_name(CONSEQUENCE_FIELD)
        .or_else(|| clause.child_by_field_name(BODY_FIELD))
}

// `find_last_nested_if`: el cuerpo es un solo `if` sin `elif` ni `else`.
fn find_last_nested_if<'tree>(body: &[Node<'tree>]) -> Option<Node<'tree>> {
    let [inner] = body else {
        return None;
    };
    if inner.kind() != IF || inner.child_by_field_name(ALTERNATIVE_FIELD).is_some() {
        return None;
    }
    find_last_nested_if(&statements(inner.child_by_field_name(CONSEQUENCE_FIELD)))
        .or_else(|| inner.child_by_field_name(CONDITION_FIELD))
}

fn is_main_check(test: Node<'_>, source: &[u8]) -> bool {
    test.kind() == COMPARISON
        && matches!(named(test).as_slice(), [name, value]
            if name.kind() == IDENTIFIER && text(*name, source) == MAIN_NAME
                && value.kind() == STRING && text_value(*value, source).as_deref() == Some(MAIN_VALUE))
}

fn mentions_system(condition: Node<'_>, source: &[u8], system: &SystemNames) -> bool {
    let mut pending = vec![condition];
    while let Some(node) = pending.pop() {
        if is_system_reference(node, source, system) {
            return true;
        }
        pending.extend(named(node));
    }
    false
}

fn is_system_reference(node: Node<'_>, source: &[u8], system: &SystemNames) -> bool {
    match node.kind() {
        ATTRIBUTE => text(node, source)
            .rsplit_once('.')
            .is_some_and(|(module, member)| {
                SYSTEM_MEMBERS.contains(&member) && system.modules.iter().any(|name| name == module)
            }),
        IDENTIFIER => system.members.iter().any(|name| name == text(node, source)),
        _ => false,
    }
}

fn system_names(nodes: &[Node<'_>], source: &[u8]) -> SystemNames {
    let mut system = SystemNames {
        modules: vec![SYS_MODULE.to_owned()],
        members: Vec::new(),
    };
    for node in nodes {
        match node.kind() {
            IMPORT => system.modules.extend(
                named(*node)
                    .into_iter()
                    .filter(|item| item.kind() == ALIASED_IMPORT)
                    .filter(|item| {
                        item.child_by_field_name(NAME_FIELD)
                            .is_some_and(|name| text(name, source) == SYS_MODULE)
                    })
                    .filter_map(|item| item.child_by_field_name(ALIAS_FIELD))
                    .map(|alias| text(alias, source).to_owned()),
            ),
            IMPORT_FROM
                if node
                    .child_by_field_name(MODULE_NAME_FIELD)
                    .is_some_and(|module| text(module, source) == SYS_MODULE) =>
            {
                system.members.extend(imported_members(*node, source));
            }
            _ => {}
        }
    }
    system
}

fn imported_members(statement: Node<'_>, source: &[u8]) -> Vec<String> {
    let mut cursor = statement.walk();
    statement
        .children_by_field_name(NAME_FIELD, &mut cursor)
        .filter_map(|name| {
            let (imported, local) = if name.kind() == ALIASED_IMPORT {
                (
                    name.child_by_field_name(NAME_FIELD)?,
                    name.child_by_field_name(ALIAS_FIELD)?,
                )
            } else {
                (name, name)
            };
            SYSTEM_MEMBERS
                .contains(&text(imported, source))
                .then(|| text(local, source).to_owned())
        })
        .collect()
}

fn is_type_checking(condition: Node<'_>, source: &[u8]) -> bool {
    let condition = unparenthesized(condition);
    match condition.kind() {
        IDENTIFIER => text(condition, source) == TYPE_CHECKING,
        ATTRIBUTE => text(condition, source).ends_with(TYPE_CHECKING),
        _ => false,
    }
}

// SIM117: un `with` cuyo único contenido es otro `with`, que se pueden escribir juntos.
fn multiple_with(statement: Node<'_>, source: &[u8]) -> Option<Report> {
    mergeable_child(statement, source)?;
    let parent = statement
        .parent()
        .filter(|parent| parent.kind() == BLOCK)
        .and_then(|block| block.parent())
        .filter(|holder| holder.kind() == WITH);
    if parent.is_some_and(|parent| mergeable_child(parent, source).is_some()) {
        return None;
    }
    Some(Report::at(PythonRule::SIM117, statement, "").with_fix(merge_with_fix(statement, source)))
}

fn mergeable_child<'tree>(outer: Node<'tree>, source: &[u8]) -> Option<Node<'tree>> {
    let body = statements(outer.child_by_field_name(BODY_FIELD));
    let [inner] = body.as_slice() else {
        return None;
    };
    let same_kind = inner.kind() == WITH && is_async(outer) == is_async(*inner);
    (same_kind && !is_timeout_scope(outer, source) && !is_timeout_scope(*inner, source)).then_some(*inner)
}

fn is_async(statement: Node<'_>) -> bool {
    let mut cursor = statement.walk();
    statement.children(&mut cursor).any(|child| child.kind() == ASYNC)
}

// Un único `asyncio.timeout(...)` y compañía, que no se mezcla con otros contextos.
fn is_timeout_scope(statement: Node<'_>, source: &[u8]) -> bool {
    let items: Vec<Node<'_>> = named(statement)
        .into_iter()
        .filter(|child| child.kind() == WITH_CLAUSE)
        .flat_map(named)
        .filter(|item| item.kind() == WITH_ITEM)
        .collect();
    let [item] = items.as_slice() else {
        return false;
    };
    item.child_by_field_name(VALUE_FIELD)
        .map(unparenthesized)
        .filter(|value| value.kind() == CALL)
        .and_then(|call| call.child_by_field_name(FUNCTION_FIELD))
        .is_some_and(|function| TIMEOUT_SCOPES.contains(&text(function, source)))
}

fn is_membership(left: Node<'_>, right: Node<'_>, source: &[u8]) -> bool {
    let between = source
        .get(left.end_byte()..right.start_byte())
        .unwrap_or_default();
    let words: Vec<&str> = std::str::from_utf8(between)
        .unwrap_or_default()
        .split_whitespace()
        .collect();
    MEMBERSHIP_OPERATORS.contains(&words.as_slice())
}

// SIM118: `clave in d.keys()`, donde alcanza con `clave in d`.
fn in_dict_keys(target: Node<'_>, iterable: Node<'_>, source: &[u8]) -> Option<Report> {
    let call = Some(unparenthesized(iterable)).filter(|call| call.kind() == CALL)?;
    let no_arguments = call
        .child_by_field_name(ARGUMENTS_FIELD)
        .is_some_and(|arguments| named(arguments).iter().all(|argument| argument.kind() == COMMENT));
    let function = call
        .child_by_field_name(FUNCTION_FIELD)
        .filter(|function| function.kind() == ATTRIBUTE)?;
    let is_keys = function
        .child_by_field_name(ATTRIBUTE_FIELD)
        .is_some_and(|name| text(name, source) == KEYS);
    let on_self = function
        .child_by_field_name(OBJECT_FIELD)
        .is_some_and(|object| object.kind() == IDENTIFIER && text(object, source) == SELF);
    if !(no_arguments && is_keys && !on_self) {
        return None;
    }
    let fix = keys_removal(call, function, source);
    Some(Report::at(PythonRule::SIM118, target, "").with_fix(fix))
}

// Borra desde el punto hasta el final de `.keys()` (con un espacio si después viene una letra); sólo
// es seguro si el objeto es un nombre que se sabe que es un `dict`.
fn keys_removal(call: Node<'_>, function: Node<'_>, source: &[u8]) -> Option<Fix> {
    let mut cursor = function.walk();
    let dot = function.children(&mut cursor).find(|child| child.kind() == DOT)?;
    let replacement = if source.get(call.end_byte()).is_some_and(u8::is_ascii_alphabetic) {
        " "
    } else {
        ""
    };
    let fix = Fix::replace_range(dot.start_position(), call.end_position(), replacement, source);
    let is_dict = function
        .child_by_field_name(OBJECT_FIELD)
        .is_some_and(|object| is_known(object, Known::Dict, source));
    Some(if is_dict { fix } else { fix.marked_unsafe() })
}
