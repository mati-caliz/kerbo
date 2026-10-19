use tree_sitter::Node;

use super::number::numeric_value;
use super::{field_is, named_children, operator, text};

const NUMBER: &str = "number";
const BIGINT_SUFFIX: char = 'n';
const SIGNS: &[&str] = &["-", "+"];
const MINUS: &str = "-";
const IGNORED_VALUES: &[f64] = &[-1.0, 0.0, 1.0, 2.0];
const MAX_ARRAY_LENGTH: f64 = 4_294_967_295.0;
const PARENTHESIZED: &str = "parenthesized_expression";
const UNARY: &str = "unary_expression";
const ENUM_MEMBER: &str = "enum_assignment";
const LITERAL_TYPE: &str = "literal_type";
const TYPE_COMBINATIONS: &[&str] = &["union_type", "intersection_type"];
const TYPE_ALIAS: &str = "type_alias_declaration";
const INDEXED_ACCESS_TYPE: &str = "lookup_type";
const CLASS_FIELD: &str = "public_field_definition";
const READONLY: &str = "readonly";
const DEFAULT_VALUES: &[(&str, &str)] = &[
    ("required_parameter", "value"),
    ("optional_parameter", "value"),
    ("object_assignment_pattern", "right"),
    ("assignment_pattern", "right"),
];
const ARGUMENTS: &str = "arguments";
const RADIX_POSITION: usize = 1;
const PARSE_INT: &str = "parseInt";
const NUMBER_PARSE_INT: &str = "Number.parseInt";
const JSX_PREFIX: &str = "jsx_";
const SUBSCRIPT: &str = "subscript_expression";
const VARIABLE_DECLARATOR: &str = "variable_declarator";
const PROPERTIES: &[&str] = &["pair", "pair_pattern"];
const COMPUTED_KEY: &str = "computed_property_name";
const OBJECT_METHOD: &str = "method_definition";
const OBJECT: &str = "object";
const ASSIGNMENTS: &[&str] = &["assignment_expression", "augmented_assignment_expression"];
const IDENTIFIER: &str = "identifier";
const FUNCTIONS: &[&str] = &[
    "function_declaration",
    "generator_function_declaration",
    "function_expression",
    "function",
    "generator_function",
    "arrow_function",
    "method_definition",
];
const CONST_DECLARATION: &str = "lexical_declaration";
const CONST_KIND: &str = "const";
const MODULE_LEVEL: &[&str] = &["program", "export_statement"];

struct Literal<'tree> {
    full: Node<'tree>,
    parent: Node<'tree>,
    value: Option<f64>,
    is_bigint: bool,
}

pub fn reported<'tree>(node: Node<'tree>, source: &[u8]) -> Option<Node<'tree>> {
    if node.kind() != NUMBER || !node.is_named() {
        return None;
    }
    let literal = literal(node, source)?;
    let exempt = is_ignored_value(&literal)
        || literal.parent.kind() == ENUM_MEMBER
        || is_literal_type_in_alias(literal.parent)
        || is_type_index(literal.parent)
        || is_readonly_field_value(&literal)
        || is_default_value(&literal)
        || is_parse_int_radix(&literal, source)
        || literal.parent.kind().starts_with(JSX_PREFIX)
        || is_array_index(&literal)
        || is_allowed_by_parent(&literal)
        || initializes_module_constant(literal.full, source);
    (!exempt).then_some(literal.full)
}

fn literal<'tree>(node: Node<'tree>, source: &[u8]) -> Option<Literal<'tree>> {
    let raw = text(node, source);
    let is_bigint = raw.ends_with(BIGINT_SUFFIX);
    let magnitude = numeric_value(raw.trim_end_matches(BIGINT_SUFFIX));
    let parent = estree_parent(node)?;
    let sign = (parent.kind() == UNARY)
        .then(|| operator(parent, source))
        .filter(|sign| SIGNS.contains(sign));
    let (full, value) = sign.map_or((node, magnitude), |sign| {
        (
            parent,
            magnitude.map(|value| if sign == MINUS { -value } else { value }),
        )
    });
    Some(Literal {
        full,
        parent: estree_parent(full)?,
        value,
        is_bigint,
    })
}

fn estree_parent(node: Node<'_>) -> Option<Node<'_>> {
    let mut parent = node.parent()?;
    while parent.kind() == PARENTHESIZED {
        parent = parent.parent()?;
    }
    Some(parent)
}

fn is_ignored_value(literal: &Literal<'_>) -> bool {
    !literal.is_bigint
        && literal.value.is_some_and(|value| {
            IGNORED_VALUES
                .iter()
                .any(|&ignored| (value - ignored).abs() < f64::EPSILON)
        })
}

fn outermost_type_combination(literal_type: Node<'_>) -> Node<'_> {
    let mut current = literal_type;
    while let Some(parent) = current
        .parent()
        .filter(|parent| TYPE_COMBINATIONS.contains(&parent.kind()))
    {
        current = parent;
    }
    current
}

fn is_literal_type_in_alias(parent: Node<'_>) -> bool {
    parent.kind() == LITERAL_TYPE
        && outermost_type_combination(parent)
            .parent()
            .is_some_and(|holder| holder.kind() == TYPE_ALIAS)
}

fn is_type_index(parent: Node<'_>) -> bool {
    parent.kind() == LITERAL_TYPE
        && outermost_type_combination(parent)
            .parent()
            .is_some_and(|holder| holder.kind() == INDEXED_ACCESS_TYPE)
}

fn is_readonly_field_value(literal: &Literal<'_>) -> bool {
    literal.parent.kind() == CLASS_FIELD
        && field_is(literal.parent, "value", literal.full)
        && named_and_anonymous_children(literal.parent)
            .iter()
            .any(|child| child.kind() == READONLY)
}

fn named_and_anonymous_children(node: Node<'_>) -> Vec<Node<'_>> {
    (0..node.child_count())
        .filter_map(|index| node.child(index))
        .collect()
}

fn is_default_value(literal: &Literal<'_>) -> bool {
    DEFAULT_VALUES
        .iter()
        .any(|&(kind, field)| literal.parent.kind() == kind && field_is(literal.parent, field, literal.full))
}

fn is_parse_int_radix(literal: &Literal<'_>, source: &[u8]) -> bool {
    if literal.parent.kind() != ARGUMENTS {
        return false;
    }
    let is_radix = named_children(literal.parent)
        .get(RADIX_POSITION)
        .is_some_and(|argument| argument.id() == literal.full.id());
    is_radix
        && literal
            .parent
            .parent()
            .and_then(|call| call.child_by_field_name("function"))
            .is_some_and(|callee| {
                let name: String = text(callee, source)
                    .chars()
                    .filter(|character| !character.is_whitespace())
                    .collect();
                name == PARSE_INT || name == NUMBER_PARSE_INT
            })
}

fn is_array_index(literal: &Literal<'_>) -> bool {
    literal.parent.kind() == SUBSCRIPT
        && field_is(literal.parent, "index", literal.full)
        && literal
            .value
            .is_some_and(|value| value.fract() == 0.0 && (0.0..MAX_ARRAY_LENGTH).contains(&value))
}

fn is_allowed_by_parent(literal: &Literal<'_>) -> bool {
    let kind = literal.parent.kind();
    if kind == VARIABLE_DECLARATOR || PROPERTIES.contains(&kind) {
        return true;
    }
    if kind == COMPUTED_KEY {
        return literal.parent.parent().is_some_and(|holder| {
            PROPERTIES.contains(&holder.kind())
                || (holder.kind() == OBJECT_METHOD
                    && holder.parent().is_some_and(|object| object.kind() == OBJECT))
        });
    }
    ASSIGNMENTS.contains(&kind)
        && literal
            .parent
            .child_by_field_name("left")
            .is_some_and(|target| target.kind() != IDENTIFIER)
}

fn initializes_module_constant(full: Node<'_>, source: &[u8]) -> bool {
    let mut current = full.parent();
    while let Some(node) = current {
        if FUNCTIONS.contains(&node.kind()) {
            return false;
        }
        if node.kind() == VARIABLE_DECLARATOR {
            return node.parent().is_some_and(|declaration| {
                declaration.kind() == CONST_DECLARATION
                    && declaration
                        .child_by_field_name("kind")
                        .is_some_and(|kind| text(kind, source) == CONST_KIND)
                    && declaration
                        .parent()
                        .is_some_and(|holder| MODULE_LEVEL.contains(&holder.kind()))
            });
        }
        current = node.parent();
    }
    false
}
