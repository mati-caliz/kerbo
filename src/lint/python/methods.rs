use tree_sitter::Node;

use super::Report;
use super::ast::{
    ASSIGNMENT, CALL, CLASS, COMMENT, DECORATED, FUNCTION, FUNCTION_FIELD, IDENTIFIER, LAMBDA, LEFT_FIELD,
    NAME_FIELD, RIGHT_FIELD, named, text, unparenthesized,
};
use crate::rules::PythonRule;

const PARAMETERS_FIELD: &str = "parameters";
const DECORATOR: &str = "decorator";
const SELF: &str = "self";
const CLS: &str = "cls";
const STATIC_DECORATORS: [&str; 4] = [
    "staticmethod",
    "builtins.staticmethod",
    "abstractstaticmethod",
    "abc.abstractstaticmethod",
];
const CLASS_DECORATORS: [&str; 4] = [
    "classmethod",
    "builtins.classmethod",
    "abstractclassmethod",
    "abc.abstractclassmethod",
];
const NEW_METHOD: &str = "__new__";
const IMPLICIT_CLASS_METHODS: [&str; 2] = ["__init_subclass__", "__class_getitem__"];
const NAMED_PARAMETERS: [&str; 3] = ["default_parameter", "typed_parameter", "typed_default_parameter"];
const CONTAINERS: [&str; 7] = [
    "tuple",
    "list",
    "pattern_list",
    "tuple_pattern",
    "list_pattern",
    "expression_list",
    "parenthesized_expression",
];
const STARRED: [&str; 2] = ["list_splat", "list_splat_pattern"];

// PLW0642: `self = ...` en un método (o `cls = ...` en un método de clase o en `__new__`), que sólo
// cambia la variable local.
pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    nodes
        .iter()
        .filter(|node| node.kind() == ASSIGNMENT)
        .filter(|assignment| {
            assignment
                .parent()
                .is_none_or(|parent| parent.kind() != ASSIGNMENT)
        })
        .flat_map(|assignment| self_or_cls_assignments(*assignment, source))
        .collect()
}

fn self_or_cls_assignments(assignment: Node<'_>, source: &[u8]) -> Vec<Report> {
    let Some(receiver) = enclosing_method_receiver(assignment, source) else {
        return Vec::new();
    };
    let mut targets = Vec::new();
    let mut current = assignment;
    loop {
        targets.extend(current.child_by_field_name(LEFT_FIELD));
        match current.child_by_field_name(RIGHT_FIELD) {
            Some(right) if right.kind() == ASSIGNMENT => current = right,
            _ => break,
        }
    }
    let mut found = Vec::new();
    for target in targets {
        receiver_targets(target, receiver, source, &mut found);
    }
    found
}

fn receiver_targets(target: Node<'_>, receiver: &str, source: &[u8], found: &mut Vec<Report>) {
    let kind = target.kind();
    if kind == IDENTIFIER {
        if text(target, source) == receiver {
            found.push(Report::at(PythonRule::PLW0642, target, ""));
        }
    } else if CONTAINERS.contains(&kind) || STARRED.contains(&kind) {
        for element in named(target)
            .into_iter()
            .filter(|element| element.kind() != COMMENT)
        {
            receiver_targets(element, receiver, source, found);
        }
    }
}

// El nombre del primer parámetro si la asignación está directamente en un método que lo recibe como
// `self` (instancia) o `cls` (de clase o `__new__`), con `function_type::classify` de ruff.
fn enclosing_method_receiver(assignment: Node<'_>, source: &[u8]) -> Option<&'static str> {
    let function = std::iter::successors(assignment.parent(), Node::parent)
        .find(|ancestor| matches!(ancestor.kind(), FUNCTION | CLASS | LAMBDA))
        .filter(|scope| scope.kind() == FUNCTION)?;
    let holder = function
        .parent()
        .filter(|parent| parent.kind() == DECORATED)
        .unwrap_or(function);
    let in_class = std::iter::successors(holder.parent(), Node::parent)
        .find(|ancestor| matches!(ancestor.kind(), FUNCTION | CLASS | LAMBDA))
        .is_some_and(|owner| owner.kind() == CLASS);
    if !in_class {
        return None;
    }
    let decorators: Vec<&str> = named(holder)
        .into_iter()
        .filter(|child| child.kind() == DECORATOR)
        .filter_map(|decorator| named(decorator).into_iter().find(|child| child.kind() != COMMENT))
        .map(|expression| {
            let expression = unparenthesized(expression);
            let callable = if expression.kind() == CALL {
                expression
                    .child_by_field_name(FUNCTION_FIELD)
                    .unwrap_or(expression)
            } else {
                expression
            };
            text(callable, source)
        })
        .collect();
    let name = text(function.child_by_field_name(NAME_FIELD)?, source);
    let first = first_parameter(function, source)?;
    let expected = if decorators
        .iter()
        .any(|decorator| STATIC_DECORATORS.contains(decorator))
    {
        return None;
    } else if decorators
        .iter()
        .any(|decorator| CLASS_DECORATORS.contains(decorator))
        || IMPLICIT_CLASS_METHODS.contains(&name)
        || name == NEW_METHOD
    {
        CLS
    } else {
        SELF
    };
    (first == expected).then_some(expected)
}

// El primer parámetro posicional (`posonlyargs` o `args` en ruff); `None` si empieza con `*`.
fn first_parameter<'source>(function: Node<'_>, source: &'source [u8]) -> Option<&'source str> {
    let first = named(function.child_by_field_name(PARAMETERS_FIELD)?)
        .into_iter()
        .find(|parameter| parameter.kind() != COMMENT)?;
    let name = match first.kind() {
        IDENTIFIER => first,
        kind if NAMED_PARAMETERS.contains(&kind) => first
            .child_by_field_name(NAME_FIELD)
            .or_else(|| named(first).into_iter().find(|child| child.kind() == IDENTIFIER))?,
        _ => return None,
    };
    Some(text(name, source))
}
