use std::collections::HashSet;

use tree_sitter::Node;

use super::Report;
use super::ast::{
    ARGUMENTS_FIELD, ASSIGNMENT, ATTRIBUTE, ATTRIBUTE_FIELD, AUGMENTED_ASSIGNMENT, BODY_FIELD, CALL,
    DECORATED, FUNCTION, FUNCTION_FIELD, IDENTIFIER, KEYWORD_ARGUMENT, LAMBDA, LEFT_FIELD, LIST, NAME_FIELD,
    OBJECT_FIELD, PARENTHESIZED, TUPLE, VALUE_FIELD, named, text, unparenthesized,
};
use crate::rules::PythonRule;

mod names;

use names::{NameRole, name_role, parameter_names};

const LOOPS: [&str; 6] = [
    "for_statement",
    "while_statement",
    "list_comprehension",
    "set_comprehension",
    "dictionary_comprehension",
    "generator_expression",
];
const FOR: &str = "for_statement";
const FOR_IN: &str = "for_in_clause";
const RETURN: &str = "return_statement";
const PARAMETERS_FIELD: &str = "parameters";
const DEFINITION_FIELD: &str = "definition";
const CONSUMING_BUILTINS: [&str; 3] = ["filter", "reduce", "map"];
const FUNCTOOLS: &str = "functools";
const REDUCE: &str = "reduce";
const KEY_KEYWORD: &str = "key";
const TARGET_CONTAINERS: [&str; 9] = [
    TUPLE,
    LIST,
    PARENTHESIZED,
    "pattern_list",
    "tuple_pattern",
    "list_pattern",
    "expression_list",
    "list_splat",
    "list_splat_pattern",
];

// B023: una función o lambda definida en un loop que usa una variable que el loop va cambiando; la
// lee cuando se llama, no cuando se define.
pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    let mut reported = HashSet::new();
    let mut reports = Vec::new();
    for loop_node in nodes.iter().filter(|node| LOOPS.contains(&node.kind())) {
        let mut suspicious = Suspicious::default();
        suspicious.visit(*loop_node, source);
        if suspicious.names.is_empty() {
            continue;
        }
        let assigned = assigned_names(*loop_node, source);
        for name in suspicious.names {
            if assigned.contains(text(name, source)) && reported.insert(name.start_byte()) {
                reports.push(Report::at(PythonRule::B023, name, ""));
            }
        }
    }
    reports
}

// `SuspiciousVariablesVisitor`: los nombres que lee una función o lambda sin ser parámetros ni
// asignarlos ella misma.
#[derive(Default)]
struct Suspicious<'tree> {
    names: Vec<Node<'tree>>,
    safe_lambdas: HashSet<usize>,
}

impl<'tree> Suspicious<'tree> {
    fn visit(&mut self, node: Node<'tree>, source: &[u8]) {
        match node.kind() {
            FUNCTION => {
                self.closure(node, node.child_by_field_name(BODY_FIELD), source);
                return;
            }
            DECORATED => {
                if let Some(definition) = node
                    .child_by_field_name(DEFINITION_FIELD)
                    .filter(|definition| definition.kind() == FUNCTION)
                {
                    self.visit(definition, source);
                    return;
                }
            }
            RETURN => {
                if let Some(lambda) = named(node).first().map(|value| unparenthesized(*value)) {
                    self.mark_safe(lambda);
                }
            }
            CALL => self.consumed_lambdas(node, source),
            LAMBDA if !self.safe_lambdas.contains(&node.id()) => {
                self.closure(node, node.child_by_field_name(BODY_FIELD), source);
                return;
            }
            _ => {}
        }
        for child in named(node) {
            self.visit(child, source);
        }
    }

    fn closure(&mut self, callable: Node<'tree>, body: Option<Node<'tree>>, source: &[u8]) {
        let Some(body) = body else {
            return;
        };
        let (loaded, stored) = loaded_and_stored(body, source);
        let parameters = parameter_names(callable.child_by_field_name(PARAMETERS_FIELD), source);
        self.names.extend(loaded.into_iter().filter(|name| {
            let name = text(*name, source);
            !stored.contains(name) && !parameters.contains(name)
        }));
    }

    fn mark_safe(&mut self, expression: Node<'_>) {
        if expression.kind() == LAMBDA {
            self.safe_lambdas.insert(expression.id());
        }
    }

    // Las lambdas que se consumen enseguida: invocadas en el acto, pasadas a `map`, `filter` o
    // `reduce`, o como `key=`.
    fn consumed_lambdas(&mut self, call: Node<'_>, source: &[u8]) {
        let Some(function) = call.child_by_field_name(FUNCTION_FIELD).map(unparenthesized) else {
            return;
        };
        self.mark_safe(function);
        let consumes = match function.kind() {
            IDENTIFIER => CONSUMING_BUILTINS.contains(&text(function, source)),
            ATTRIBUTE => {
                function
                    .child_by_field_name(ATTRIBUTE_FIELD)
                    .is_some_and(|attribute| text(attribute, source) == REDUCE)
                    && function.child_by_field_name(OBJECT_FIELD).is_some_and(|object| {
                        object.kind() == IDENTIFIER && text(object, source) == FUNCTOOLS
                    })
            }
            _ => false,
        };
        let arguments = call
            .child_by_field_name(ARGUMENTS_FIELD)
            .map(named)
            .unwrap_or_default();
        for argument in arguments {
            let safe_lambda = if argument.kind() == KEYWORD_ARGUMENT {
                argument
                    .child_by_field_name(VALUE_FIELD)
                    .filter(|_| is_key_keyword(argument, source))
            } else {
                consumes.then_some(argument)
            };
            if let Some(safe_lambda) = safe_lambda {
                self.mark_safe(unparenthesized(safe_lambda));
            }
        }
    }
}

// `LoadedNamesVisitor`: los nombres que se leen y los que se asignan en todo el cuerpo.
fn loaded_and_stored<'tree, 'source>(
    body: Node<'tree>,
    source: &'source [u8],
) -> (Vec<Node<'tree>>, HashSet<&'source str>) {
    let mut loaded = Vec::new();
    let mut stored = HashSet::new();
    let mut pending = vec![body];
    while let Some(node) = pending.pop() {
        if node.kind() == IDENTIFIER {
            match name_role(node) {
                NameRole::Load => loaded.push(node),
                NameRole::Store => {
                    stored.insert(text(node, source));
                }
                NameRole::Other => {}
            }
        }
        pending.extend(named(node));
    }
    (loaded, stored)
}

// `AssignedNamesVisitor`: lo que el loop asigna, sin entrar a funciones ni lambdas.
fn assigned_names<'source>(loop_node: Node<'_>, source: &'source [u8]) -> HashSet<&'source str> {
    let mut assigned = HashSet::new();
    let mut pending = vec![loop_node];
    while let Some(node) = pending.pop() {
        match node.kind() {
            FUNCTION | LAMBDA => continue,
            DECORATED
                if node
                    .child_by_field_name(DEFINITION_FIELD)
                    .is_some_and(|definition| definition.kind() == FUNCTION) =>
            {
                continue;
            }
            ASSIGNMENT | AUGMENTED_ASSIGNMENT | FOR | FOR_IN => {
                if let Some(target) = node.child_by_field_name(LEFT_FIELD) {
                    target_names(target, source, &mut assigned);
                }
            }
            _ => {}
        }
        pending.extend(named(node));
    }
    assigned
}

// `NamesFromAssignmentsVisitor`: los nombres sueltos de un destino, también dentro de tuplas y listas.
fn target_names<'source>(target: Node<'_>, source: &'source [u8], names: &mut HashSet<&'source str>) {
    if target.kind() == IDENTIFIER {
        names.insert(text(target, source));
    } else if TARGET_CONTAINERS.contains(&target.kind()) {
        for element in named(target) {
            target_names(element, source, names);
        }
    }
}

fn is_key_keyword(argument: Node<'_>, source: &[u8]) -> bool {
    argument
        .child_by_field_name(NAME_FIELD)
        .is_some_and(|name| text(name, source) == KEY_KEYWORD)
}
