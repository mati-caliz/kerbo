use tree_sitter::Node;

use super::Report;
use super::ast::{
    ASSIGNMENT, ATTRIBUTE, ATTRIBUTE_FIELD, BLOCK, BODY_FIELD, CLASS, COMMENT, EXPRESSION_STATEMENT,
    FUNCTION, IDENTIFIER, KEYWORD_ARGUMENT, LAMBDA, LEFT_FIELD, LIST, NAME_FIELD, RIGHT_FIELD, TUPLE,
    TYPE_FIELD, named, text, unparenthesized,
};
use super::semantic::is_dummy;
use crate::rules::PythonRule;

const UNARY: &str = "unary_operator";
const OPERATOR_FIELD: &str = "operator";
const ARGUMENT_FIELD: &str = "argument";
const DOUBLED_OPERATORS: [&str; 2] = ["+", "-"];
const TUPLES: [&str; 4] = [TUPLE, "pattern_list", "expression_list", "tuple_pattern"];
const TARGET_CONTAINERS: [&str; 8] = [
    TUPLE,
    LIST,
    "pattern_list",
    "tuple_pattern",
    "list_pattern",
    "expression_list",
    "list_splat",
    "list_splat_pattern",
];
const DICTIONARY: &str = "dictionary";
const PARAMETERS_FIELD: &str = "parameters";
const OS_MODULE: &str = "os";
const ENVIRON: &str = "environ";
const OBJECT_FIELD: &str = "object";

pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    nodes
        .iter()
        .flat_map(|node| match node.kind() {
            UNARY => doubled_unary(*node, source).into_iter().collect(),
            ASSIGNMENT => {
                let mut found = self_assignments(*node, source);
                found.extend(redeclared_names(*node, source));
                found.extend(environ_assignment(*node, source));
                found
            }
            CLASS => duplicate_class_fields(*node, source),
            LAMBDA => empty_container_lambda(*node).into_iter().collect(),
            _ => Vec::new(),
        })
        .collect()
}

// B002: `++x` o `--x`, que en Python no incrementa ni decrementa: son dos signos seguidos.
fn doubled_unary(operation: Node<'_>, source: &[u8]) -> Option<Report> {
    let operator = text(operation.child_by_field_name(OPERATOR_FIELD)?, source);
    let operand = operation
        .child_by_field_name(ARGUMENT_FIELD)
        .map(unparenthesized)
        .filter(|operand| operand.kind() == UNARY)?;
    let inner = text(operand.child_by_field_name(OPERATOR_FIELD)?, source);
    (DOUBLED_OPERATORS.contains(&operator) && operator == inner)
        .then(|| Report::at(PythonRule::B002, operation, ""))
}

// PLW0127: `x = x` (también adentro de tuplas y en asignaciones encadenadas), fuera del cuerpo de
// una clase, donde sí puede copiar un nombre de afuera.
fn self_assignments(assignment: Node<'_>, source: &[u8]) -> Vec<Report> {
    let is_chain_link = assignment
        .parent()
        .is_some_and(|parent| parent.kind() == ASSIGNMENT);
    if is_chain_link || is_in_class_scope(assignment) {
        return Vec::new();
    }
    let mut sides = Vec::new();
    let mut current = assignment;
    loop {
        sides.extend(current.child_by_field_name(LEFT_FIELD));
        match current.child_by_field_name(RIGHT_FIELD) {
            Some(right) if right.kind() == ASSIGNMENT => current = right,
            Some(right) => {
                sides.push(right);
                break;
            }
            None => break,
        }
    }
    let mut found = Vec::new();
    for (index, left) in sides.iter().enumerate() {
        for right in sides.iter().skip(index.saturating_add(1)) {
            same_names(*left, *right, source, &mut found);
        }
    }
    found
}

fn same_names<'tree>(left: Node<'tree>, right: Node<'tree>, source: &[u8], found: &mut Vec<Report>) {
    let (left, right) = (unparenthesized(left), unparenthesized(right));
    if TUPLES.contains(&left.kind()) && TUPLES.contains(&right.kind()) {
        let (lefts, rights) = (named(left), named(right));
        if lefts.len() == rights.len() {
            for (left_element, right_element) in lefts.into_iter().zip(rights) {
                same_names(left_element, right_element, source, found);
            }
        }
    } else if left.kind() == IDENTIFIER
        && right.kind() == IDENTIFIER
        && text(left, source) == text(right, source)
    {
        found.push(Report::at(PythonRule::PLW0127, left, ""));
    }
}

fn is_in_class_scope(node: Node<'_>) -> bool {
    std::iter::successors(node.parent(), Node::parent)
        .find(|ancestor| matches!(ancestor.kind(), CLASS | FUNCTION | LAMBDA))
        .is_some_and(|scope| scope.kind() == CLASS)
}

// PLW0128: `a, a = 1, 2`, el mismo nombre dos veces entre los destinos de una asignación. Avisa en la
// segunda aparición; los nombres descartables (`_x`) no cuentan.
fn redeclared_names(assignment: Node<'_>, source: &[u8]) -> Vec<Report> {
    let is_chain_link = assignment
        .parent()
        .is_some_and(|parent| parent.kind() == ASSIGNMENT);
    if is_chain_link || assignment.child_by_field_name(TYPE_FIELD).is_some() {
        return Vec::new();
    }
    let mut targets = Vec::new();
    let mut current = assignment;
    loop {
        targets.extend(current.child_by_field_name(LEFT_FIELD));
        match current.child_by_field_name(RIGHT_FIELD) {
            Some(right) if right.kind() == ASSIGNMENT => current = right,
            _ => break,
        }
    }
    let mut seen = Vec::new();
    let mut found = Vec::new();
    for target in targets {
        repeated_names(target, source, &mut seen, &mut found);
    }
    found
}

fn repeated_names<'source>(
    target: Node<'_>,
    source: &'source [u8],
    seen: &mut Vec<&'source str>,
    found: &mut Vec<Report>,
) {
    let target = unparenthesized(target);
    if target.kind() == IDENTIFIER {
        let name = text(target, source);
        if is_dummy(name) {
            return;
        }
        if seen.contains(&name) {
            found.push(Report::at(PythonRule::PLW0128, target, ""));
        }
        seen.push(name);
    } else if TARGET_CONTAINERS.contains(&target.kind()) {
        for element in named(target)
            .into_iter()
            .filter(|element| element.kind() != COMMENT)
        {
            repeated_names(element, source, seen, found);
        }
    }
}

// PIE794: el mismo atributo definido dos veces en el cuerpo de una clase (sin contar las que leen
// el valor anterior, como `x = x + 1`).
fn duplicate_class_fields(class: Node<'_>, source: &[u8]) -> Vec<Report> {
    let Some(body) = class
        .child_by_field_name(BODY_FIELD)
        .filter(|body| body.kind() == BLOCK)
    else {
        return Vec::new();
    };
    let mut seen = Vec::new();
    let mut found = Vec::new();
    for statement in named(body)
        .into_iter()
        .filter(|statement| statement.kind() == EXPRESSION_STATEMENT)
    {
        let Some(assignment) = named(statement)
            .into_iter()
            .find(|child| child.kind() == ASSIGNMENT)
        else {
            continue;
        };
        let Some(target) = assignment
            .child_by_field_name(LEFT_FIELD)
            .filter(|target| target.kind() == IDENTIFIER)
        else {
            continue;
        };
        let value = assignment.child_by_field_name(RIGHT_FIELD);
        if value.is_some_and(|value| value.kind() == ASSIGNMENT) {
            continue;
        }
        let name = text(target, source);
        if value.is_some_and(|value| reads_name(value, name, source)) {
            continue;
        }
        if seen.contains(&name) {
            found.push(Report::at(PythonRule::PIE794, statement, ""));
        } else {
            seen.push(name);
        }
    }
    found
}

fn reads_name(node: Node<'_>, name: &str, source: &[u8]) -> bool {
    if node.kind() == IDENTIFIER {
        let is_member = node.parent().is_some_and(|parent| {
            parent.kind() == ATTRIBUTE
                && parent
                    .child_by_field_name(ATTRIBUTE_FIELD)
                    .map(|field| field.id())
                    == Some(node.id())
                || parent.kind() == KEYWORD_ARGUMENT
                    && parent.child_by_field_name(NAME_FIELD).map(|field| field.id()) == Some(node.id())
        });
        return !is_member && text(node, source) == name;
    }
    named(node)
        .into_iter()
        .any(|child| reads_name(child, name, source))
}

// PIE807: `lambda: []` o `lambda: {}`, que es `list` o `dict`.
fn empty_container_lambda(lambda: Node<'_>) -> Option<Report> {
    if lambda.child_by_field_name(PARAMETERS_FIELD).is_some() {
        return None;
    }
    let body = lambda.child_by_field_name(BODY_FIELD).map(unparenthesized)?;
    let is_empty_container =
        matches!(body.kind(), LIST | DICTIONARY) && named(body).iter().all(|child| child.kind() == COMMENT);
    is_empty_container.then(|| Report::at(PythonRule::PIE807, lambda, ""))
}

// B003: `os.environ = {...}`, que reemplaza el objeto en vez de cambiar el entorno del proceso.
fn environ_assignment(assignment: Node<'_>, source: &[u8]) -> Option<Report> {
    let is_single_target = assignment
        .parent()
        .is_none_or(|parent| parent.kind() != ASSIGNMENT)
        && assignment
            .child_by_field_name(RIGHT_FIELD)
            .is_none_or(|right| right.kind() != ASSIGNMENT);
    let target = assignment
        .child_by_field_name(LEFT_FIELD)
        .filter(|target| target.kind() == ATTRIBUTE)?;
    let is_environ = target
        .child_by_field_name(ATTRIBUTE_FIELD)
        .is_some_and(|attribute| text(attribute, source) == ENVIRON)
        && target
            .child_by_field_name(OBJECT_FIELD)
            .is_some_and(|object| object.kind() == IDENTIFIER && text(object, source) == OS_MODULE);
    (is_single_target && is_environ && assignment.child_by_field_name(TYPE_FIELD).is_none())
        .then(|| Report::at(PythonRule::B003, target, ""))
}
