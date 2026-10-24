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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MethodKind {
    Instance,
    Class,
    New,
    Static,
}

// PLW0642: `self = ...` en un método (o `cls = ...` en un método de clase o en `__new__`), que sólo
// cambia la variable local. PLW0211: un método estático (o `__new__`) cuyo primer parámetro se
// llama `self` o `cls`.
pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    nodes
        .iter()
        .flat_map(|node| match node.kind() {
            ASSIGNMENT if node.parent().is_none_or(|parent| parent.kind() != ASSIGNMENT) => {
                self_or_cls_assignments(*node, source)
            }
            FUNCTION => bad_static_argument(*node, source).into_iter().collect(),
            _ => Vec::new(),
        })
        .collect()
}

fn self_or_cls_assignments(assignment: Node<'_>, source: &[u8]) -> Vec<Report> {
    let Some(function) = std::iter::successors(assignment.parent(), Node::parent)
        .find(|ancestor| matches!(ancestor.kind(), FUNCTION | CLASS | LAMBDA))
        .filter(|scope| scope.kind() == FUNCTION)
    else {
        return Vec::new();
    };
    let receiver = match (method_kind(function, source), first_parameter(function)) {
        (Some(MethodKind::Instance), Some(first)) if text(first, source) == SELF => SELF,
        (Some(MethodKind::Class | MethodKind::New), Some(first)) if text(first, source) == CLS => CLS,
        _ => return Vec::new(),
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

// PLW0211: avisa en el parámetro.
fn bad_static_argument(function: Node<'_>, source: &[u8]) -> Option<Report> {
    let kind = method_kind(function, source)?;
    if !matches!(kind, MethodKind::Static | MethodKind::New) {
        return None;
    }
    let first = first_parameter(function)?;
    let name = text(function.child_by_field_name(NAME_FIELD)?, source);
    let parameter = text(first, source);
    let allowed = name == NEW_METHOD && parameter == CLS;
    (!allowed && (parameter == SELF || parameter == CLS)).then(|| Report::at(PythonRule::PLW0211, first, ""))
}

// `function_type::classify` de ruff, con los decoradores reconocidos por nombre; `None` si la
// función no está definida directamente en una clase.
fn method_kind(function: Node<'_>, source: &[u8]) -> Option<MethodKind> {
    let holder = function
        .parent()
        .filter(|parent| parent.kind() == DECORATED)
        .unwrap_or(function);
    std::iter::successors(holder.parent(), Node::parent)
        .find(|ancestor| matches!(ancestor.kind(), FUNCTION | CLASS | LAMBDA))
        .filter(|owner| owner.kind() == CLASS)?;
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
    let kind = if decorators
        .iter()
        .any(|decorator| STATIC_DECORATORS.contains(decorator))
    {
        MethodKind::Static
    } else if decorators
        .iter()
        .any(|decorator| CLASS_DECORATORS.contains(decorator))
        || IMPLICIT_CLASS_METHODS.contains(&name)
    {
        MethodKind::Class
    } else if name == NEW_METHOD {
        MethodKind::New
    } else {
        MethodKind::Instance
    };
    Some(kind)
}

// El primer parámetro posicional (`posonlyargs` o `args` en ruff); `None` si empieza con `*`.
fn first_parameter(function: Node<'_>) -> Option<Node<'_>> {
    let first = named(function.child_by_field_name(PARAMETERS_FIELD)?)
        .into_iter()
        .find(|parameter| parameter.kind() != COMMENT)?;
    match first.kind() {
        IDENTIFIER => Some(first),
        kind if NAMED_PARAMETERS.contains(&kind) => first
            .child_by_field_name(NAME_FIELD)
            .or_else(|| named(first).into_iter().find(|child| child.kind() == IDENTIFIER)),
        _ => None,
    }
}
