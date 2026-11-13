use tree_sitter::Node;

use super::java::text;
use crate::syntax::named_children;

const LOCAL: &str = "local_variable_declaration";
const VARIABLE_DECLARATOR: &str = "variable_declarator";
const ASSIGNMENT: &str = "assignment_expression";
const BINARY: &str = "binary_expression";
const PARENTHESIZED: &str = "parenthesized_expression";
const FIELD_ACCESS: &str = "field_access";
const IF: &str = "if_statement";
const BLOCK: &str = "block";
const ENDING_EXITS: &[&str] = &["return_statement", "throw_statement"];
const COMMENTS: &[&str] = &["line_comment", "block_comment"];
const INIT_FIELD: &str = "init";
const CONDITION_FIELD: &str = "condition";
const CONSEQUENCE_FIELD: &str = "consequence";
const ALTERNATIVE_FIELD: &str = "alternative";
const NAME_FIELD: &str = "name";
const VALUE_FIELD: &str = "value";
const LEFT_FIELD: &str = "left";
const RIGHT_FIELD: &str = "right";
const OPERATOR_FIELD: &str = "operator";
const OBJECT_FIELD: &str = "object";
const FIELD_FIELD: &str = "field";
const LENGTH: &str = "length";
const ZERO: &str = "0";
const LESS: &str = "<";
const EQUAL: &str = "==";

// Si el `for (i = 0; i < arreglo.length; ...)` da al menos una vuelta porque antes, en el mismo
// bloque, un `if (arreglo.length == 0)` sale del método: IntelliJ sigue el largo del arreglo.
pub(super) fn runs_at_least_once(for_statement: Node<'_>, source: &[u8]) -> bool {
    let Some(counter) = counter_from_zero(for_statement, source) else {
        return false;
    };
    let Some(array) = bounding_array(for_statement, &counter, source) else {
        return false;
    };
    let mut previous = for_statement.prev_named_sibling();
    while let Some(statement) = previous {
        if exits_when_empty(statement, &array, source) {
            return true;
        }
        previous = statement.prev_named_sibling();
    }
    false
}

fn counter_from_zero(for_statement: Node<'_>, source: &[u8]) -> Option<String> {
    let init = for_statement.child_by_field_name(INIT_FIELD)?;
    let (name, value) = match init.kind() {
        LOCAL => {
            let declarator = named_children(init)
                .into_iter()
                .find(|child| child.kind() == VARIABLE_DECLARATOR)?;
            (
                declarator.child_by_field_name(NAME_FIELD)?,
                declarator.child_by_field_name(VALUE_FIELD)?,
            )
        }
        ASSIGNMENT => (
            init.child_by_field_name(LEFT_FIELD)?,
            init.child_by_field_name(RIGHT_FIELD)?,
        ),
        _ => return None,
    };
    (text(value, source) == ZERO).then(|| text(name, source).to_owned())
}

fn bounding_array(for_statement: Node<'_>, counter: &str, source: &[u8]) -> Option<String> {
    let condition = unparenthesized(for_statement.child_by_field_name(CONDITION_FIELD)?);
    let (left, operator, right) = binary_parts(condition, source)?;
    if operator != LESS || text(left, source) != counter {
        return None;
    }
    length_of(right, source)
}

fn exits_when_empty(statement: Node<'_>, array: &str, source: &[u8]) -> bool {
    if statement.kind() != IF || statement.child_by_field_name(ALTERNATIVE_FIELD).is_some() {
        return false;
    }
    let compares_to_empty = statement
        .child_by_field_name(CONDITION_FIELD)
        .map(unparenthesized)
        .and_then(|condition| binary_parts(condition, source))
        .is_some_and(|(left, operator, right)| {
            operator == EQUAL
                && length_of(left, source).as_deref() == Some(array)
                && text(right, source) == ZERO
        });
    compares_to_empty
        && statement
            .child_by_field_name(CONSEQUENCE_FIELD)
            .is_some_and(ends_with_exit)
}

fn ends_with_exit(statement: Node<'_>) -> bool {
    if statement.kind() == BLOCK {
        return named_children(statement)
            .into_iter()
            .rfind(|child| !COMMENTS.contains(&child.kind()))
            .is_some_and(ends_with_exit);
    }
    ENDING_EXITS.contains(&statement.kind())
}

fn binary_parts<'tree>(node: Node<'tree>, source: &[u8]) -> Option<(Node<'tree>, String, Node<'tree>)> {
    if node.kind() != BINARY {
        return None;
    }
    Some((
        node.child_by_field_name(LEFT_FIELD)?,
        text(node.child_by_field_name(OPERATOR_FIELD)?, source).to_owned(),
        node.child_by_field_name(RIGHT_FIELD)?,
    ))
}

fn length_of(node: Node<'_>, source: &[u8]) -> Option<String> {
    let is_length = node.kind() == FIELD_ACCESS
        && node
            .child_by_field_name(FIELD_FIELD)
            .is_some_and(|field| text(field, source) == LENGTH);
    if !is_length {
        return None;
    }
    node.child_by_field_name(OBJECT_FIELD)
        .map(|object| text(object, source).to_owned())
}

fn unparenthesized(node: Node<'_>) -> Node<'_> {
    let mut current = node;
    while current.kind() == PARENTHESIZED {
        match current.named_child(0) {
            Some(inner) => current = inner,
            None => break,
        }
    }
    current
}
