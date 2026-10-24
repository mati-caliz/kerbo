use tree_sitter::Node;

use super::ast::{
    ATTRIBUTE, CLASS, FUNCTION, IDENTIFIER, LAMBDA, LEFT_FIELD, MODULE, NAME_FIELD, named, text,
};

const CONTAINERS: [&str; 8] = [
    "pattern_list",
    "tuple_pattern",
    "list_pattern",
    "tuple",
    "list",
    "parenthesized_expression",
    "list_splat_pattern",
    "expression_list",
];
const SCOPES: [&str; 4] = [FUNCTION, CLASS, LAMBDA, MODULE];
const BINDING_PARENTS: [&str; 10] = [
    "as_pattern_target",
    "parameters",
    "lambda_parameters",
    "typed_parameter",
    "dotted_name",
    "aliased_import",
    "import_statement",
    "import_from_statement",
    "global_statement",
    "nonlocal_statement",
];
const LEFT_BINDERS: [&str; 3] = ["assignment", "for_statement", "for_in_clause"];
const NAMED_BINDERS: [&str; 6] = [
    FUNCTION,
    CLASS,
    "keyword_argument",
    "default_parameter",
    "typed_default_parameter",
    "named_expression",
];
const ATTRIBUTE_FIELD: &str = "attribute";
const FOR_IN_CLAUSE: &str = "for_in_clause";
const COMPREHENSIONS: [&str; 4] = [
    "list_comprehension",
    "set_comprehension",
    "dictionary_comprehension",
    "generator_expression",
];

// `is_unused` de ruff, aproximado sobre el árbol: el nombre se lee en algún lugar del mismo alcance,
// también a través de otra variable con el mismo nombre ligada antes o después.
pub fn is_read_in_scope(name: Node<'_>, source: &[u8]) -> bool {
    let Some(scope) =
        std::iter::successors(name.parent(), Node::parent).find(|ancestor| SCOPES.contains(&ancestor.kind()))
    else {
        return false;
    };
    let wanted = text(name, source);
    let mut pending = vec![scope];
    while let Some(node) = pending.pop() {
        if node.kind() == IDENTIFIER {
            if node.id() != name.id() && text(node, source) == wanted && is_load(node) {
                return true;
            }
            continue;
        }
        if !rebinds(node, wanted, source) && !is_own_scope(node, name, wanted, source) {
            pending.extend(named(node));
        }
    }
    false
}

// Una comprensión con su propia variable del mismo nombre: lo que lee adentro es esa variable.
fn rebinds(node: Node<'_>, wanted: &str, source: &[u8]) -> bool {
    COMPREHENSIONS.contains(&node.kind())
        && named(node)
            .into_iter()
            .filter(|clause| clause.kind() == FOR_IN_CLAUSE)
            .filter_map(|clause| clause.child_by_field_name(LEFT_FIELD))
            .any(|target| binds_name(target, wanted, source))
}

// Una función, lambda o clase de adentro que liga el mismo nombre: lo que lee es su propia variable.
fn is_own_scope(node: Node<'_>, name: Node<'_>, wanted: &str, source: &[u8]) -> bool {
    if !SCOPES.contains(&node.kind())
        || node.start_byte() <= name.start_byte() && name.end_byte() <= node.end_byte()
    {
        return false;
    }
    let mut pending = named(node);
    while let Some(inner) = pending.pop() {
        if inner.kind() == IDENTIFIER {
            if text(inner, source) == wanted && !is_load(inner) {
                return true;
            }
            continue;
        }
        pending.extend(named(inner));
    }
    false
}

fn binds_name(target: Node<'_>, wanted: &str, source: &[u8]) -> bool {
    if target.kind() == IDENTIFIER {
        return text(target, source) == wanted;
    }
    named(target)
        .into_iter()
        .any(|inner| binds_name(inner, wanted, source))
}

pub fn is_load(identifier: Node<'_>) -> bool {
    let mut child = identifier;
    let mut parent = identifier.parent();
    while let Some(container) = parent.filter(|container| CONTAINERS.contains(&container.kind())) {
        child = container;
        parent = container.parent();
    }
    let Some(parent) = parent else {
        return true;
    };
    let is_field = |field: &str| {
        parent
            .child_by_field_name(field)
            .is_some_and(|value| value.id() == child.id())
    };
    match parent.kind() {
        kind if BINDING_PARENTS.contains(&kind) => false,
        kind if LEFT_BINDERS.contains(&kind) => !is_field(LEFT_FIELD),
        kind if NAMED_BINDERS.contains(&kind) => !is_field(NAME_FIELD),
        ATTRIBUTE => !is_field(ATTRIBUTE_FIELD),
        _ => true,
    }
}
