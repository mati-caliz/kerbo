use tree_sitter::Node;

use super::Report;
use super::ast::{
    ASSIGNMENT, ATTRIBUTE, CLASS, COMMENT, FALSE, FUNCTION, IDENTIFIER, LAMBDA, LEFT_FIELD, MODULE,
    RIGHT_FIELD, SUBSCRIPT, TRUE, TUPLE, VALUE_FIELD, is_string, named, string_prefix, text, unparenthesized,
};
use super::escapes::text_value;
use super::known_types::is_binding;
use crate::rules::PythonRule;

const BODY_FIELD: &str = "body";
const ATTRIBUTE_FIELD: &str = "attribute";
const OBJECT_FIELD: &str = "object";
const KEY_FIELD: &str = "key";
const EXPRESSION_STATEMENT: &str = "expression_statement";
const GLOBAL: &str = "global_statement";
const NONLOCAL: &str = "nonlocal_statement";
const NAMED_EXPRESSION: &str = "named_expression";
const PARENTHESIZED: &str = "parenthesized_expression";
const EXCEPT: &str = "except_clause";
const AS_PATTERN: &str = "as_pattern";
const LIST_SPLAT: &str = "list_splat";
const FINALLY: &str = "finally_clause";
const JUMPS: [&str; 3] = ["return_statement", "break_statement", "continue_statement"];
const RETURN: &str = "return_statement";
const LOOPS: [&str; 2] = ["for_statement", "while_statement"];
const DICTIONARY_COMPREHENSION: &str = "dictionary_comprehension";
const FOR_IN_CLAUSE: &str = "for_in_clause";
const PAIR: &str = "pair";
const ALL: &str = "__all__";
const SCOPES: [&str; 3] = [FUNCTION, CLASS, LAMBDA];
const NONE_LITERAL: &str = "none";
const STATIC_KINDS: [&str; 6] = ["integer", "float", TRUE, FALSE, NONE_LITERAL, "ellipsis"];
const STATIC_PARTS: [&str; 6] = [
    TUPLE,
    ATTRIBUTE,
    SUBSCRIPT,
    "binary_operator",
    "boolean_operator",
    "unary_operator",
];
const FORMATTED_PREFIXES: [char; 2] = ['f', 'F'];
const CONCATENATED_STRING: &str = "concatenated_string";
const VALID_ALL: [&str; 4] = ["list", TUPLE, "expression_list", "list_comprehension"];
const BINARY_OPERATOR: &str = "binary_operator";

// Chequeos sobre sentencias: D419 y B021 (docstrings), PLW0602, PLW0131, B013, B012, B035 y PLE0605.
pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    let mut found = Vec::new();
    for node in nodes {
        match node.kind() {
            MODULE | FUNCTION | CLASS => found.extend(docstring(*node, source)),
            GLOBAL => found.extend(unassigned_globals(*node, source)),
            NAMED_EXPRESSION => found.extend(named_without_context(*node)),
            EXCEPT => found.extend(redundant_tuple(*node)),
            kind if JUMPS.contains(&kind) => found.extend(jump_in_finally(*node)),
            DICTIONARY_COMPREHENSION => found.extend(static_key(*node, source)),
            ASSIGNMENT => found.extend(invalid_all(*node, source)),
            _ => {}
        }
    }
    found
}

fn significant(node: Node<'_>) -> Vec<Node<'_>> {
    named(node)
        .into_iter()
        .filter(|child| child.kind() != COMMENT)
        .collect()
}

// D419 (docstring vacío) y B021 (f-string como docstring, que Python no toma como tal).
fn docstring(owner: Node<'_>, source: &[u8]) -> Option<Report> {
    let body = if owner.kind() == MODULE {
        owner
    } else {
        owner.child_by_field_name(BODY_FIELD)?
    };
    let first = *significant(body).first()?;
    if first.kind() != EXPRESSION_STATEMENT {
        return None;
    }
    let parts_1 = significant(first);
    let [string] = parts_1.as_slice() else {
        return None;
    };
    if !is_string(*string) {
        return None;
    }
    let parts = if string.kind() == CONCATENATED_STRING {
        significant(*string)
    } else {
        vec![*string]
    };
    if parts
        .iter()
        .any(|part| string_prefix(*part, source).contains(FORMATTED_PREFIXES))
    {
        return Some(Report::at(PythonRule::B021, *string, ""));
    }
    text_value(*string, source)
        .filter(|value| value.trim().is_empty())
        .map(|_| Report::at(PythonRule::D419, *string, ""))
}

// PLW0602: `global x` en una función o clase que nunca asigna `x`. Avisa en el nombre.
fn unassigned_globals(statement: Node<'_>, source: &[u8]) -> Vec<Report> {
    let Some(function) = std::iter::successors(statement.parent(), Node::parent)
        .find(|ancestor| SCOPES.contains(&ancestor.kind()) || ancestor.kind() == MODULE)
        .filter(|scope| matches!(scope.kind(), FUNCTION | CLASS))
    else {
        return Vec::new();
    };
    significant(statement)
        .into_iter()
        .filter(|name| name.kind() == IDENTIFIER)
        .filter(|name| {
            !assigns(function, text(*name, source), source) && !declared_again(function, *name, source)
        })
        .map(|name| Report::at(PythonRule::PLW0602, name, ""))
        .collect()
}

fn declared_again(scope: Node<'_>, name: Node<'_>, source: &[u8]) -> bool {
    let mut pending = named(scope);
    while let Some(node) = pending.pop() {
        if node.kind() == GLOBAL && node.start_byte() > name.end_byte() {
            if significant(node)
                .iter()
                .any(|other| text(*other, source) == text(name, source))
            {
                return true;
            }
            continue;
        }
        if !SCOPES.contains(&node.kind()) || node.id() == scope.id() {
            pending.extend(named(node));
        }
    }
    false
}

fn assigns(function: Node<'_>, wanted: &str, source: &[u8]) -> bool {
    let mut pending = function
        .child_by_field_name(BODY_FIELD)
        .map(named)
        .unwrap_or_default();
    while let Some(node) = pending.pop() {
        if node.kind() == IDENTIFIER {
            let declared = node
                .parent()
                .is_some_and(|parent| matches!(parent.kind(), GLOBAL | NONLOCAL));
            if text(node, source) == wanted && is_binding(node) && !declared {
                return true;
            }
            continue;
        }
        if SCOPES.contains(&node.kind()) {
            if node
                .child_by_field_name("name")
                .is_some_and(|name| text(name, source) == wanted)
            {
                return true;
            }
            continue;
        }
        pending.extend(named(node));
    }
    false
}

// PLW0131: `(x := 1)` como sentencia, donde alcanza con `x = 1`.
fn named_without_context(expression: Node<'_>) -> Option<Report> {
    let parent = std::iter::successors(expression.parent(), Node::parent)
        .find(|ancestor| ancestor.kind() != PARENTHESIZED)?;
    (parent.kind() == EXPRESSION_STATEMENT).then(|| Report::at(PythonRule::PLW0131, expression, ""))
}

// B013: `except (ValueError,):`, una tupla de un solo elemento.
fn redundant_tuple(handler: Node<'_>) -> Option<Report> {
    let value = handler.child_by_field_name(VALUE_FIELD)?;
    let caught = if value.kind() == AS_PATTERN {
        *significant(value).first()?
    } else {
        value
    };
    let parts_2 = significant(caught);
    let [only] = parts_2.as_slice() else {
        return None;
    };
    (caught.kind() == TUPLE && only.kind() != LIST_SPLAT).then(|| Report::at(PythonRule::B013, caught, ""))
}

// B012: `return`, `break` o `continue` adentro de un `finally`, que se traga la excepción.
fn jump_in_finally(statement: Node<'_>) -> Option<Report> {
    let mut crossed_loop = false;
    for ancestor in std::iter::successors(statement.parent(), Node::parent) {
        match ancestor.kind() {
            FINALLY => {
                return (statement.kind() == RETURN || !crossed_loop)
                    .then(|| Report::at(PythonRule::B012, statement, ""));
            }
            kind if LOOPS.contains(&kind) => crossed_loop = true,
            kind if SCOPES.contains(&kind) => return None,
            _ => {}
        }
    }
    None
}

// B035: una comprensión de diccionario cuya clave no depende de lo que recorre.
fn static_key(comprehension: Node<'_>, source: &[u8]) -> Option<Report> {
    let pair = comprehension
        .child_by_field_name(BODY_FIELD)
        .filter(|pair| pair.kind() == PAIR)?;
    let key = pair.child_by_field_name(KEY_FIELD)?;
    let mut bound = Vec::new();
    for clause in significant(comprehension)
        .into_iter()
        .filter(|clause| clause.kind() == FOR_IN_CLAUSE)
    {
        if let Some(target) = clause.child_by_field_name(LEFT_FIELD) {
            collect_names(target, source, &mut bound);
        }
    }
    is_static(key, &bound, source).then(|| Report::at(PythonRule::B035, key, ""))
}

fn collect_names<'source>(target: Node<'_>, source: &'source [u8], names: &mut Vec<&'source str>) {
    if target.kind() == IDENTIFIER {
        names.push(text(target, source));
        return;
    }
    for inner in named(target) {
        collect_names(inner, source, names);
    }
}

fn is_static(key: Node<'_>, bound: &[&str], source: &[u8]) -> bool {
    let key = unparenthesized(key);
    match key.kind() {
        IDENTIFIER => !bound.contains(&text(key, source)),
        kind if STATIC_KINDS.contains(&kind) => true,
        _ if is_string(key) => !string_prefix(key, source).contains(FORMATTED_PREFIXES),
        ATTRIBUTE => key
            .child_by_field_name(OBJECT_FIELD)
            .is_some_and(|object| is_static(object, bound, source)),
        kind if STATIC_PARTS.contains(&kind) => {
            let parts: Vec<Node<'_>> = significant(key)
                .into_iter()
                .filter(|part| !(key.kind() == ATTRIBUTE && part.kind() == IDENTIFIER))
                .collect();
            !parts.is_empty() && parts.iter().all(|part| is_static(*part, bound, source))
        }
        _ => false,
    }
}

// PLE0605: `__all__` asignado a algo que no es una lista ni una tupla.
fn invalid_all(assignment: Node<'_>, source: &[u8]) -> Option<Report> {
    let target = assignment.child_by_field_name(LEFT_FIELD)?;
    let value = unparenthesized(assignment.child_by_field_name(RIGHT_FIELD)?);
    (text(target, source) == ALL && !is_valid_all(value, source))
        .then(|| Report::at(PythonRule::PLE0605, assignment, ""))
}

fn is_valid_all(value: Node<'_>, source: &[u8]) -> bool {
    let value = unparenthesized(value);
    VALID_ALL.contains(&value.kind())
        || (value.kind() == ATTRIBUTE
            && value
                .child_by_field_name(ATTRIBUTE_FIELD)
                .is_some_and(|attribute| text(attribute, source) == ALL))
        || (value.kind() == BINARY_OPERATOR
            && [LEFT_FIELD, RIGHT_FIELD].into_iter().all(|side| {
                value
                    .child_by_field_name(side)
                    .is_some_and(|operand| is_valid_all(operand, source))
            }))
}
