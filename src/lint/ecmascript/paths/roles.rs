use tree_sitter::Node;

use super::state::{ChoiceKind, LoopKind};
use crate::lint::ecmascript::nodes::{
    ARGUMENTS, BINARY, CALL, COMMENT, IDENTIFIER, MEMBER, NUMBER, STRING, SUBSCRIPT, operator, text,
    without_parentheses,
};
use crate::lint::ecmascript::strings::{numeric_text, static_string};

pub const OPTIONAL_CHAIN_FIELD: &str = "optional_chain";
pub const LOGICAL_ASSIGNMENT: &str = "augmented_assignment_expression";
pub const ASSIGNMENT_PATTERNS: &[&str] = &["assignment_pattern", "object_assignment_pattern"];
pub const DEFAULTED_PARAMETERS: &[&str] = &["required_parameter", "optional_parameter"];
pub const SWITCH_CASES: &[&str] = &["switch_case", "switch_default"];
pub const SWITCH_DEFAULT: &str = "switch_default";
pub const LABELED: &str = "labeled_statement";
pub const CODE_PATH_FUNCTIONS: &[&str] = &[
    "function_declaration",
    "function_expression",
    "function",
    "generator_function_declaration",
    "generator_function",
    "arrow_function",
];
pub const STATIC_BLOCK: &str = "class_static_block";
pub const METHOD: &str = "method_definition";
pub const FIELD_DEFINITIONS: &[&str] = &["field_definition", "public_field_definition"];
pub const NEW: &str = "new_expression";
const TEST_FIELD: &str = "condition";
const VALUE_FIELD: &str = "value";
const NAME_FIELD: &str = "name";
const KEY_FIELD: &str = "key";
const PROPERTY_FIELD: &str = "property";
const ARGUMENTS_FIELD: &str = "arguments";
const OBJECT_FIELD: &str = "object";
const FUNCTION_FIELD: &str = "function";
const TEMPLATE_STRING: &str = "template_string";
const TESTED: &[&str] = &[
    "ternary_expression",
    "if_statement",
    "while_statement",
    "do_statement",
    "for_statement",
];
const IDENTIFIERS: &[&str] = &[
    IDENTIFIER,
    "property_identifier",
    "shorthand_property_identifier",
    "shorthand_property_identifier_pattern",
    "type_identifier",
    "statement_identifier",
    "undefined",
];
const NOT_REFERENCE_PARENTS: &[&str] = &[
    "array_pattern",
    "rest_pattern",
    "import_specifier",
    "import_clause",
    "namespace_import",
    "catch_clause",
    "statement_identifier",
];
const NAMED_DECLARATIONS: &[&str] = &[
    "function_declaration",
    "function_expression",
    "function",
    "generator_function_declaration",
    "generator_function",
    "class_declaration",
    "abstract_class_declaration",
    "class",
    "variable_declarator",
    METHOD,
    "field_definition",
    "public_field_definition",
];
const KEYED: &[&str] = &["pair", "pair_pattern"];
const CHAIN_LINKS: &[&str] = &[MEMBER, SUBSCRIPT, CALL];
const BREAKABLE: &[&str] = &[
    "while_statement",
    "do_statement",
    "for_statement",
    "for_in_statement",
    "switch_statement",
];
const AND: &str = "&&";
const OR: &str = "||";
const NULLISH: &str = "??";
const LOGICAL_ASSIGNMENTS: &[(&str, ChoiceKind)] = &[
    ("&&=", ChoiceKind::And),
    ("||=", ChoiceKind::Or),
    ("??=", ChoiceKind::Nullish),
];

pub fn logical_kind(node: Node<'_>, source: &[u8]) -> Option<ChoiceKind> {
    match node.kind() {
        BINARY => match operator(node, source) {
            AND => Some(ChoiceKind::And),
            OR => Some(ChoiceKind::Or),
            NULLISH => Some(ChoiceKind::Nullish),
            _ => None,
        },
        LOGICAL_ASSIGNMENT => {
            let operator = operator(node, source);
            LOGICAL_ASSIGNMENTS
                .iter()
                .find(|(candidate, _)| *candidate == operator)
                .map(|(_, kind)| *kind)
        }
        _ => None,
    }
}

// Dónde está un nodo: su padre y el campo que ocupa en él (sin paréntesis, como en ESTree, cuando
// se pide así). Se arma durante el recorrido porque `Node::parent` de tree-sitter busca desde la raíz.
#[derive(Debug, Clone, Copy, Default)]
pub struct Place<'tree> {
    pub parent: Option<Node<'tree>>,
    pub field: Option<&'tree str>,
}

impl Place<'_> {
    pub fn parent_kind(&self) -> &str {
        self.parent.map_or("", |parent| parent.kind())
    }

    pub fn is(&self, field: &str) -> bool {
        self.field == Some(field)
    }
}

// `isForkingByTrueOrFalse`: el resultado de la expresión lógica decide una bifurcación de arriba.
pub fn forks_by_result(estree: Place<'_>, source: &[u8]) -> bool {
    let Some(parent) = estree.parent else {
        return false;
    };
    if TESTED.contains(&parent.kind()) {
        return estree.is(TEST_FIELD);
    }
    logical_kind(parent, source).is_some()
}

// `getBooleanValueIfSimpleConstant`: sólo un literal decide la condición de un loop.
pub fn simple_constant(test: Node<'_>, source: &[u8]) -> Option<bool> {
    let test = without_parentheses(test);
    match test.kind() {
        "true" | "regex" => Some(true),
        "false" | "null" => Some(false),
        STRING => static_string(test, source).map(|value| !value.is_empty()),
        NUMBER => Some(numeric_text(text(test, source)).is_none_or(|value| value != "0")),
        _ => None,
    }
}

pub fn loop_kind(kind: &str) -> Option<LoopKind> {
    match kind {
        "while_statement" => Some(LoopKind::While),
        "do_statement" => Some(LoopKind::DoWhile),
        "for_statement" => Some(LoopKind::For),
        "for_in_statement" => Some(LoopKind::ForIn),
        _ => None,
    }
}

pub fn label_of(place: Place<'_>, source: &[u8]) -> Option<String> {
    place
        .parent
        .filter(|parent| parent.kind() == LABELED)
        .and_then(|labeled| labeled.child_by_field_name("label"))
        .map(|label| text(label, source).to_owned())
}

pub fn is_breakable(kind: &str) -> bool {
    BREAKABLE.contains(&kind)
}

pub fn is_optional(node: Node<'_>) -> bool {
    node.child_by_field_name(OPTIONAL_CHAIN_FIELD).is_some()
}

pub fn is_tagged_template(call: Node<'_>) -> bool {
    call.child_by_field_name(ARGUMENTS_FIELD)
        .is_some_and(|arguments| arguments.kind() == TEMPLATE_STRING)
}

pub fn first_argument(call: Node<'_>) -> Option<Node<'_>> {
    let arguments = call
        .child_by_field_name(ARGUMENTS_FIELD)
        .filter(|arguments| arguments.kind() == ARGUMENTS)?;
    let mut cursor = arguments.walk();
    arguments
        .named_children(&mut cursor)
        .find(|child| child.kind() != COMMENT)
}

// Un `ChainExpression` de ESTree: el último eslabón de una cadena con `?.`.
pub fn is_chain_root(node: Node<'_>, place: Place<'_>) -> bool {
    if !CHAIN_LINKS.contains(&node.kind()) || (node.kind() == CALL && is_tagged_template(node)) {
        return false;
    }
    let mut current = Some(node);
    let mut optional = false;
    while let Some(link) = current.filter(|link| CHAIN_LINKS.contains(&link.kind())) {
        optional = optional || is_optional(link);
        let field = if link.kind() == CALL {
            FUNCTION_FIELD
        } else {
            OBJECT_FIELD
        };
        current = link.child_by_field_name(field);
    }
    let continued_by = if place.parent_kind() == CALL {
        FUNCTION_FIELD
    } else {
        OBJECT_FIELD
    };
    optional && !(CHAIN_LINKS.contains(&place.parent_kind()) && place.is(continued_by))
}

pub fn is_identifier(kind: &str) -> bool {
    IDENTIFIERS.contains(&kind)
}

// `isIdentifierReference` sobre los nodos de tree-sitter.
pub fn is_identifier_reference(node: Node<'_>, place: Place<'_>) -> bool {
    if node.kind() == "statement_identifier" {
        return false;
    }
    let parent = place.parent_kind();
    if NOT_REFERENCE_PARENTS.contains(&parent) {
        return false;
    }
    if NAMED_DECLARATIONS.contains(&parent) {
        let name_field = if FIELD_DEFINITIONS.contains(&parent) {
            PROPERTY_FIELD
        } else {
            NAME_FIELD
        };
        return !place.is(name_field);
    }
    if KEYED.contains(&parent) {
        return !place.is(KEY_FIELD);
    }
    true
}

pub fn is_field_value(place: Place<'_>) -> bool {
    FIELD_DEFINITIONS.contains(&place.parent_kind()) && place.is(VALUE_FIELD)
}

// Los hijos con nombre (sin comentarios) y el campo que ocupa cada uno.
pub fn children_with_fields(node: Node<'_>) -> Vec<(Node<'_>, Option<&str>)> {
    let mut children = Vec::new();
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.is_named() && child.kind() != COMMENT {
                children.push((child, cursor.field_name()));
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
    children
}
