use std::collections::HashSet;

use tree_sitter::Node;

use super::java::text;
use crate::syntax::named_children;

const IDENTIFIER: &str = "identifier";
const OBJECT_FIELD: &str = "object";
const DEREFERENCES: &[&str] = &["method_invocation", "field_access"];
const ARRAY_ACCESS: &str = "array_access";
const ARRAY_FIELD: &str = "array";
const CONDITION_FIELD: &str = "condition";
const INIT_FIELD: &str = "init";
const VALUE_FIELD: &str = "value";
const RESOURCES_FIELD: &str = "resources";
const LOCK_FIELD: &str = "lock";
const BLOCK: &str = "block";
const IF: &str = "if_statement";
const LOOPS_WITH_CONDITION: &[&str] = &["while_statement", "for_statement"];
const ENHANCED_FOR: &str = "enhanced_for_statement";
const TRY_STATEMENTS: &[&str] = &["try_statement", "try_with_resources_statement"];
const SYNCHRONIZED: &str = "synchronized_statement";
const SWITCH: &str = "switch_expression";
const PLAIN_STATEMENTS: &[&str] = &[
    "expression_statement",
    "local_variable_declaration",
    "return_statement",
    "throw_statement",
];
// Lo que puede no ejecutarse aunque se ejecute la expresión que lo contiene.
const TERNARY: &str = "ternary_expression";
const CONDITIONAL_PARTS: &[&str] = &["lambda_expression", "class_body", TERNARY, "switch_block"];
const SHORT_CIRCUIT: &[&str] = &["&&", "||"];
const BINARY: &str = "binary_expression";
const OPERATOR_FIELD: &str = "operator";
const LEFT_FIELD: &str = "left";
const EARLY_EXITS: &[&str] = &[
    "return_statement",
    "throw_statement",
    "break_statement",
    "continue_statement",
    "yield_statement",
];
const COMMENTS: &[&str] = &["line_comment", "block_comment"];

// Los nombres que el cuerpo de un método desreferencia (`p.algo()`, `p.campo`, `p[0]`) en todos
// los caminos: IntelliJ infiere de eso que el parámetro es no nulo. Se recorren las sentencias en
// orden hasta la primera que puede salir antes o un `try` (de él sólo cuentan los recursos); de un
// `if` o un bucle cuenta la condición, no el cuerpo, y de una expresión no cuentan las lambdas,
// las ramas de un ternario ni lo que está a la derecha de un `&&` o un `||`.
pub fn unconditionally_dereferenced(body: Node<'_>, source: &[u8]) -> HashSet<String> {
    let mut found = HashSet::new();
    for statement in statements(body) {
        if !visit_statement(statement, source, &mut found) {
            break;
        }
    }
    found
}

fn statements(block: Node<'_>) -> Vec<Node<'_>> {
    named_children(block)
        .into_iter()
        .filter(|child| !COMMENTS.contains(&child.kind()))
        .collect()
}

// Junta lo desreferenciado en la sentencia y dice si las siguientes se ejecutan seguro.
fn visit_statement(statement: Node<'_>, source: &[u8], found: &mut HashSet<String>) -> bool {
    let kind = statement.kind();
    if PLAIN_STATEMENTS.contains(&kind) {
        collect(statement, source, found);
        return !EARLY_EXITS.contains(&kind);
    }
    if kind == BLOCK {
        return statements(statement)
            .into_iter()
            .all(|inner| visit_statement(inner, source, found));
    }
    let evaluated: Vec<Node<'_>> = if kind == IF || LOOPS_WITH_CONDITION.contains(&kind) || kind == SWITCH {
        [CONDITION_FIELD, INIT_FIELD]
            .iter()
            .filter_map(|field| statement.child_by_field_name(field))
            .collect()
    } else if kind == ENHANCED_FOR {
        statement.child_by_field_name(VALUE_FIELD).into_iter().collect()
    } else if kind == SYNCHRONIZED {
        statement.child_by_field_name(LOCK_FIELD).into_iter().collect()
    } else if TRY_STATEMENTS.contains(&kind) {
        if let Some(resources) = statement.child_by_field_name(RESOURCES_FIELD) {
            collect(resources, source, found);
        }
        return false;
    } else {
        Vec::new()
    };
    for part in evaluated {
        collect(part, source, found);
    }
    !exits_early(statement)
}

fn exits_early(statement: Node<'_>) -> bool {
    let mut pending = vec![statement];
    while let Some(node) = pending.pop() {
        if EARLY_EXITS.contains(&node.kind()) {
            return true;
        }
        if !CONDITIONAL_PARTS.contains(&node.kind()) || node == statement {
            pending.extend(named_children(node));
        }
    }
    false
}

fn collect(expression: Node<'_>, source: &[u8], found: &mut HashSet<String>) {
    let mut pending = vec![expression];
    while let Some(node) = pending.pop() {
        let kind = node.kind();
        if CONDITIONAL_PARTS.contains(&kind) {
            if kind == TERNARY {
                pending.extend(node.child_by_field_name(CONDITION_FIELD));
            }
            continue;
        }
        if kind == BINARY
            && node
                .child_by_field_name(OPERATOR_FIELD)
                .is_some_and(|operator| SHORT_CIRCUIT.contains(&operator.kind()))
        {
            pending.extend(node.child_by_field_name(LEFT_FIELD));
            continue;
        }
        let receiver = if DEREFERENCES.contains(&kind) {
            node.child_by_field_name(OBJECT_FIELD)
        } else if kind == ARRAY_ACCESS {
            node.child_by_field_name(ARRAY_FIELD)
        } else {
            None
        };
        if let Some(name) = receiver.filter(|receiver| receiver.kind() == IDENTIFIER) {
            found.insert(text(name, source).to_owned());
        }
        pending.extend(named_children(node));
    }
}
