use tree_sitter::Node;

use super::ast::{
    ASSIGNMENT, CALL, CLASS, FUNCTION, FUNCTION_FIELD, IDENTIFIER, LAMBDA, LEFT_FIELD, MODULE, NAME_FIELD,
    RIGHT_FIELD, TYPE_FIELD, named, text,
};

const SCOPES: [&str; 4] = [FUNCTION, CLASS, LAMBDA, MODULE];
const TYPED_PARAMETERS: [&str; 2] = ["typed_parameter", "typed_default_parameter"];
const AUGMENTED: &str = "augmented_assignment";
const DELETE: &str = "delete_statement";
const BINDING_PARENTS: [&str; 11] = [
    "as_pattern_target",
    "parameters",
    "lambda_parameters",
    "typed_parameter",
    "aliased_import",
    "dotted_name",
    "global_statement",
    "nonlocal_statement",
    "list_splat_pattern",
    DICTIONARY_SPLAT_PATTERN,
    NAMED_EXPRESSION,
];
const CONTAINERS: [&str; 6] = [
    "pattern_list",
    "tuple_pattern",
    "list_pattern",
    "tuple",
    "list",
    "parenthesized_expression",
];
const LEFT_BINDERS: [&str; 3] = [ASSIGNMENT, "for_statement", "for_in_clause"];
const DICTIONARY_SPLAT_PATTERN: &str = "dictionary_splat_pattern";
const NAMED_EXPRESSION: &str = "named_expression";
const SINGLE_TYPES: [&str; 4] = [IDENTIFIER, "attribute", "generic_type", "subscript"];
const UNPACKED_TARGETS: [&str; 3] = ["pattern_list", "tuple_pattern", "tuple"];
const UNPACKED_VALUES: [&str; 2] = ["expression_list", "tuple"];
const NAMED_PARAMETERS: [&str; 2] = ["default_parameter", "typed_default_parameter"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Known {
    Dict,
    List,
}

impl Known {
    const fn literals(self) -> [&'static str; 2] {
        match self {
            Self::Dict => ["dictionary", "dictionary_comprehension"],
            Self::List => ["list", "list_comprehension"],
        }
    }

    const fn names(self) -> [&'static str; 2] {
        match self {
            Self::Dict => ["dict", "Dict"],
            Self::List => ["list", "List"],
        }
    }
}

// `typing::is_dict` o `is_list` de ruff sobre la única variable con ese nombre (`only_binding`): un
// literal, una llamada a `dict()` o `list()`, una anotación con ese único tipo o `**kwargs`. En el
// alcance del nombre cuentan las asignaciones anteriores (ruff lo mira al recorrer); si ahí no se
// liga, el alcance que lo contiene.
pub fn is_known(name: Node<'_>, kind: Known, source: &[u8]) -> bool {
    if name.kind() != IDENTIFIER {
        return false;
    }
    let wanted = text(name, source);
    for (depth, scope) in std::iter::successors(name.parent(), Node::parent)
        .filter(|ancestor| SCOPES.contains(&ancestor.kind()))
        .enumerate()
    {
        let mut bindings = bindings_in(scope, wanted, source);
        if depth == 0 {
            bindings.retain(|binding| binding.start_byte() < name.start_byte());
        }
        match bindings.as_slice() {
            [] => {}
            [only] => return binds_known(*only, kind, source),
            _ => return false,
        }
    }
    false
}

fn bindings_in<'tree>(scope: Node<'tree>, wanted: &str, source: &[u8]) -> Vec<Node<'tree>> {
    let mut found = Vec::new();
    let mut pending: Vec<Node<'tree>> = named(scope);
    if scope.kind() == FUNCTION {
        pending.retain(|child| child.kind() != IDENTIFIER);
    }
    while let Some(node) = pending.pop() {
        if node.kind() == IDENTIFIER {
            if text(node, source) == wanted && is_binding(node) {
                found.push(node);
            }
            continue;
        }
        if SCOPES.contains(&node.kind()) {
            if let Some(name) = node
                .child_by_field_name(NAME_FIELD)
                .filter(|name| text(*name, source) == wanted)
            {
                found.push(name);
            }
            continue;
        }
        pending.extend(named(node));
    }
    found
}

pub fn is_binding(identifier: Node<'_>) -> bool {
    let mut child = identifier;
    let mut parent = identifier.parent();
    while let Some(container) = parent.filter(|container| CONTAINERS.contains(&container.kind())) {
        child = container;
        parent = container.parent();
    }
    let Some(parent) = parent else {
        return false;
    };
    let is_field = |field: &str| {
        parent
            .child_by_field_name(field)
            .is_some_and(|value| value.id() == child.id())
    };
    match parent.kind() {
        kind if LEFT_BINDERS.contains(&kind) => is_field(LEFT_FIELD),
        AUGMENTED => is_field(LEFT_FIELD),
        DELETE => true,
        kind if NAMED_PARAMETERS.contains(&kind) => is_field(NAME_FIELD),
        kind if BINDING_PARENTS.contains(&kind) => true,
        _ => false,
    }
}

fn binds_known(binding: Node<'_>, kind: Known, source: &[u8]) -> bool {
    let Some(parent) = binding.parent() else {
        return false;
    };
    if parent.kind() == DICTIONARY_SPLAT_PATTERN {
        return kind == Known::Dict;
    }
    if let Some(value) = unpacked_value(binding) {
        return is_known_value(value, kind, source);
    }
    if TYPED_PARAMETERS.contains(&parent.kind()) {
        return parent
            .child_by_field_name(TYPE_FIELD)
            .is_some_and(|annotation| is_known_annotation(annotation, kind, source));
    }
    if parent.kind() != ASSIGNMENT
        || parent
            .child_by_field_name(LEFT_FIELD)
            .is_none_or(|target| target.id() != binding.id())
    {
        return false;
    }
    if let Some(annotation) = parent.child_by_field_name(TYPE_FIELD)
        && is_known_annotation(annotation, kind, source)
    {
        return true;
    }
    parent
        .child_by_field_name(RIGHT_FIELD)
        .is_some_and(|value| is_known_value(value, kind, source))
}

fn is_known_value(value: Node<'_>, kind: Known, source: &[u8]) -> bool {
    kind.literals().contains(&value.kind())
        || (value.kind() == CALL
            && value
                .child_by_field_name(FUNCTION_FIELD)
                .is_some_and(|function| text(function, source) == kind.names()[0]))
}

// `a, b = x, []`: el valor que le toca a `binding` en una asignación desempaquetada.
fn unpacked_value(binding: Node<'_>) -> Option<Node<'_>> {
    let targets = binding
        .parent()
        .filter(|targets| UNPACKED_TARGETS.contains(&targets.kind()))?;
    let assignment = targets.parent().filter(|assignment| {
        assignment.kind() == ASSIGNMENT
            && assignment
                .child_by_field_name(LEFT_FIELD)
                .is_some_and(|left| left.id() == targets.id())
    })?;
    let values = assignment
        .child_by_field_name(RIGHT_FIELD)
        .filter(|values| UNPACKED_VALUES.contains(&values.kind()))?;
    let index = named(targets)
        .iter()
        .position(|target| target.id() == binding.id())?;
    let elements = named(values);
    (elements.len() == named(targets).len())
        .then(|| elements.get(index).copied())
        .flatten()
}

fn is_known_annotation(annotation: Node<'_>, kind: Known, source: &[u8]) -> bool {
    let inner = named(annotation).first().copied().unwrap_or(annotation);
    if !SINGLE_TYPES.contains(&inner.kind()) {
        return false;
    }
    let written = text(annotation, source);
    let base = written.split('[').next().unwrap_or(written).trim();
    let last = base.rsplit('.').next().unwrap_or(base);
    kind.names().contains(&last)
}
