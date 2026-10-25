use tree_sitter::Node;

use super::Report;
use super::ast::{
    ASSIGNMENT, AUGMENTED_ASSIGNMENT, BLOCK, BODY_FIELD, CLASS, COMMENT, DECORATED, EXPRESSION_STATEMENT,
    FUNCTION, IDENTIFIER, LAMBDA, LEFT_FIELD, MODULE, RIGHT_FIELD, TYPE_FIELD, named, text, unparenthesized,
};
use super::known_types::{is_binding, is_name_reference, is_read_after};
use crate::rules::PythonRule;

const AWAIT: &str = "await";
const ASYNC: &str = "async";
const FOR: &str = "for_statement";
const WITH: &str = "with_statement";
const FOR_IN_CLAUSE: &str = "for_in_clause";
const GENERATOR: &str = "generator_expression";
const SYNC_COMPREHENSIONS: [&str; 3] = [
    "list_comprehension",
    "set_comprehension",
    "dictionary_comprehension",
];
const CONTINUE: &str = "continue_statement";
const GLOBAL: &str = "global_statement";
const NONLOCAL: &str = "nonlocal_statement";
const ELSE: &str = "else_clause";
const SCOPES: [&str; 4] = [FUNCTION, CLASS, LAMBDA, MODULE];
const DEFINITIONS: [&str; 3] = [FUNCTION, CLASS, DECORATED];
const INCREMENT: &str = "+=";
const OPERATOR_FIELD: &str = "operator";
const ONE: &str = "1";
const ZERO: &str = "0";

// PLE1142 (`await` fuera de una función async) y SIM113 (un contador a mano en vez de `enumerate`).
pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    let mut found = Vec::new();
    for node in nodes {
        match node.kind() {
            AWAIT if node.is_named() => found.extend(await_outside_async(*node, *node)),
            FOR | WITH if starts_with_async(*node, source) => found.extend(await_outside_async(*node, *node)),
            FOR_IN_CLAUSE
                if starts_with_async(*node, source)
                    && node
                        .parent()
                        .is_some_and(|parent| SYNC_COMPREHENSIONS.contains(&parent.kind())) =>
            {
                found.extend(await_outside_async(*node, *node));
            }
            FOR if !starts_with_async(*node, source) => found.extend(manual_counters(*node, source)),
            _ => {}
        }
    }
    found
}

fn starts_with_async(node: Node<'_>, source: &[u8]) -> bool {
    node.child(0).is_some_and(|first| text(first, source) == ASYNC)
}

fn significant(node: Node<'_>) -> Vec<Node<'_>> {
    named(node)
        .into_iter()
        .filter(|child| child.kind() != COMMENT)
        .collect()
}

// PLE1142: la función más cercana (sin contar lambdas ni clases) no es async. Un generador se evalúa
// tarde, así que adentro de uno vale, salvo en el primer iterable.
fn await_outside_async(node: Node<'_>, reported: Node<'_>) -> Option<Report> {
    let mut child = node;
    let mut in_generator = false;
    for ancestor in std::iter::successors(node.parent(), Node::parent) {
        if ancestor.kind() == GENERATOR && !is_first_iterable(ancestor, child) {
            in_generator = true;
        }
        if ancestor.kind() == FUNCTION {
            let is_async = ancestor.child(0).is_some_and(|first| first.kind() == ASYNC);
            return (!is_async && !in_generator).then(|| Report::at(PythonRule::PLE1142, reported, ""));
        }
        child = ancestor;
    }
    (!in_generator).then(|| Report::at(PythonRule::PLE1142, reported, ""))
}

fn is_first_iterable(generator: Node<'_>, child: Node<'_>) -> bool {
    named(generator)
        .into_iter()
        .find(|part| part.kind() == FOR_IN_CLAUSE)
        .and_then(|clause| clause.child_by_field_name(RIGHT_FIELD))
        .is_some_and(|iterable| {
            iterable.start_byte() <= child.start_byte() && child.end_byte() <= iterable.end_byte()
        })
}

// SIM113: `i = 0` antes del `for` y `i += 1` en su cuerpo. Como ruff: sin `continue` en el cuerpo, el
// `i = 0` en el mismo bloque que el loop, `i` ligado una sola vez adentro y sin leerse después.
fn manual_counters(statement: Node<'_>, source: &[u8]) -> Vec<Report> {
    let Some(body) = statement.child_by_field_name(BODY_FIELD) else {
        return Vec::new();
    };
    if contains_continue(body) {
        return Vec::new();
    }
    significant(body)
        .into_iter()
        .filter_map(|increment| {
            let name = incremented_name(increment, source)?;
            is_manual_counter(statement, increment, name, source)
                .then(|| Report::at(PythonRule::SIM113, increment, name))
        })
        .collect()
}

fn contains_continue(node: Node<'_>) -> bool {
    named(node).into_iter().any(|child| {
        child.kind() == CONTINUE || (!DEFINITIONS.contains(&child.kind()) && contains_continue(child))
    })
}

fn incremented_name<'source>(statement: Node<'_>, source: &'source [u8]) -> Option<&'source str> {
    let augmented = significant(statement)
        .first()
        .copied()
        .filter(|inner| statement.kind() == EXPRESSION_STATEMENT && inner.kind() == AUGMENTED_ASSIGNMENT)?;
    let is_increment = augmented
        .child_by_field_name(OPERATOR_FIELD)
        .is_some_and(|operator| text(operator, source) == INCREMENT);
    let target = augmented
        .child_by_field_name(LEFT_FIELD)
        .filter(|target| target.kind() == IDENTIFIER)?;
    let adds_one = augmented
        .child_by_field_name(RIGHT_FIELD)
        .is_some_and(|value| value.kind() == "integer" && text(value, source) == ONE);
    (is_increment && adds_one).then(|| text(target, source))
}

fn is_manual_counter(statement: Node<'_>, increment: Node<'_>, name: &str, source: &[u8]) -> bool {
    let Some(scope) =
        std::iter::successors(statement.parent(), Node::parent).find(|node| SCOPES.contains(&node.kind()))
    else {
        return false;
    };
    let mut bindings = Vec::new();
    let mut declared = false;
    collect_uses(scope, name, source, &mut |identifier, binding| {
        if binding {
            bindings.push(identifier);
        }
    });
    collect_declarations(scope, name, source, &mut declared);
    if is_read_after(statement, name, source) || declared {
        return false;
    }
    let inside = |node: Node<'_>| {
        statement.start_byte() <= node.start_byte() && node.end_byte() <= statement.end_byte()
    };
    if bindings.iter().filter(|binding| inside(**binding)).count() != 1 {
        return false;
    }
    let Some(initial) = bindings
        .iter()
        .filter(|binding| binding.start_byte() < increment.start_byte())
        .max_by_key(|binding| binding.start_byte())
    else {
        return false;
    };
    let Some(assignment) = initial
        .parent()
        .filter(|parent| parent.kind() == ASSIGNMENT && parent.child_by_field_name(TYPE_FIELD).is_none())
    else {
        return false;
    };
    let starts_at_zero = assignment
        .child_by_field_name(RIGHT_FIELD)
        .map(unparenthesized)
        .is_some_and(|value| value.kind() == "integer" && text(value, source) == ZERO);
    let same_block = assignment
        .parent()
        .and_then(|holder| holder.parent())
        .zip(statement.parent())
        .is_some_and(|(first, second)| first.id() == second.id() && matches!(first.kind(), BLOCK | MODULE));
    starts_at_zero && same_block && !in_else(statement, *initial)
}

fn in_else(statement: Node<'_>, node: Node<'_>) -> bool {
    named(statement)
        .into_iter()
        .filter(|child| child.kind() == ELSE)
        .any(|clause| clause.start_byte() <= node.start_byte() && node.end_byte() <= clause.end_byte())
}

// Las apariciones del nombre en el alcance, también en funciones de adentro: `binding` dice si lo liga.
fn collect_uses<'tree>(
    node: Node<'tree>,
    name: &str,
    source: &[u8],
    visit: &mut dyn FnMut(Node<'tree>, bool),
) {
    for child in named(node) {
        if child.kind() == IDENTIFIER {
            if text(child, source) == name && is_name_reference(child) {
                let nested_definition = child
                    .parent()
                    .is_some_and(|parent| DEFINITIONS.contains(&parent.kind()) || parent.kind() == CLASS);
                visit(child, is_binding(child) && !nested_definition);
            }
            continue;
        }
        collect_uses(child, name, source, visit);
    }
}

fn collect_declarations(node: Node<'_>, name: &str, source: &[u8], declared: &mut bool) {
    for child in named(node) {
        if matches!(child.kind(), GLOBAL | NONLOCAL) {
            *declared |= named(child)
                .iter()
                .any(|declared_name| text(*declared_name, source) == name);
        } else if !DEFINITIONS.contains(&child.kind()) {
            collect_declarations(child, name, source, declared);
        }
    }
}
