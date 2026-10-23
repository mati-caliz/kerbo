use tree_sitter::Node;

use super::constants::Constants;
use super::members::static_key_name;
use super::nodes::{
    ARGUMENTS_FIELD, CALL, COMMENT, FUNCTION_FIELD, IDENTIFIER, KEY_FIELD, NAME_FIELD, VALUE_FIELD,
    first_named_child, text, without_parentheses,
};
use super::{Context, report};
use crate::lint::LintFinding;
use crate::rules::EslintRule;

const THROW: &str = "throw_statement";
const CATCH: &str = "catch_clause";
const PARAMETER_FIELD: &str = "parameter";
const NEW: &str = "new_expression";
const CONSTRUCTOR_FIELD: &str = "constructor";
const OPTIONAL_CHAIN_FIELD: &str = "optional_chain";
const OBJECT: &str = "object";
const SPREAD: &str = "spread_element";
const PAIR: &str = "pair";
const SHORTHAND: &str = "shorthand_property_identifier";
const METHOD: &str = "method_definition";
const PARAMETERS_FIELD: &str = "parameters";
const CAUSE: &str = "cause";
const AGGREGATE_ERROR: &str = "AggregateError";
const AGGREGATE_OPTIONS_INDEX: usize = 2;
const OPTIONS_INDEX: usize = 1;
const BUILT_IN_ERRORS: &[&str] = &[
    "Error",
    "EvalError",
    "RangeError",
    "ReferenceError",
    "SyntaxError",
    "TypeError",
    "URIError",
    AGGREGATE_ERROR,
];
const CATCH_BOUNDARIES: &[&str] = &[
    "function_declaration",
    "function_expression",
    "function",
    "generator_function_declaration",
    "generator_function",
    "method_definition",
    "arrow_function",
    "class_static_block",
];

enum Cause<'tree> {
    Missing,
    Unknown,
    Given(Node<'tree>),
}

// `preserve-caught-error`: un error nuevo tirado desde un `catch` sin el original en `cause`.
pub fn lost_caught_errors(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let constants = Constants::new(context);
    context
        .of_kind(&[THROW])
        .filter_map(|throw| lost_error(throw, &constants, context))
        .map(|node| report(EslintRule::PreserveCaughtError, node, ""))
        .collect()
}

fn lost_error<'tree>(
    throw: Node<'tree>,
    constants: &Constants<'_, 'tree, '_>,
    context: &Context<'tree, '_>,
) -> Option<Node<'tree>> {
    let catch = enclosing_catch(throw)?;
    let thrown = first_named_child(throw).map(without_parentheses)?;
    let error_name = new_built_in_error(thrown, constants, context)?;
    let parameter = catch.child_by_field_name(PARAMETER_FIELD)?;
    if parameter.kind() != IDENTIFIER {
        return Some(catch);
    }
    let caught = text(parameter, context.source);
    let options_index = if error_name == AGGREGATE_ERROR {
        AGGREGATE_OPTIONS_INDEX
    } else {
        OPTIONS_INDEX
    };
    match error_cause(thrown, options_index, context) {
        Cause::Unknown => None,
        Cause::Missing => Some(throw),
        Cause::Given(value) => {
            let value = without_parentheses(value);
            if !(value.kind() == IDENTIFIER || value.kind() == SHORTHAND)
                || text(value, context.source) != caught
            {
                return Some(value);
            }
            is_shadowed(throw, catch, caught, context).then_some(throw)
        }
    }
}

fn enclosing_catch(throw: Node<'_>) -> Option<Node<'_>> {
    let mut current = throw.parent();
    while let Some(node) = current {
        if node.kind() == CATCH {
            return Some(node);
        }
        if CATCH_BOUNDARIES.contains(&node.kind()) {
            return None;
        }
        current = node.parent();
    }
    None
}

fn new_built_in_error<'source>(
    thrown: Node<'_>,
    constants: &Constants<'_, '_, '_>,
    context: &Context<'_, 'source>,
) -> Option<&'source str> {
    let callee = match thrown.kind() {
        NEW => thrown.child_by_field_name(CONSTRUCTOR_FIELD),
        CALL if thrown.child_by_field_name(OPTIONAL_CHAIN_FIELD).is_none() => {
            thrown.child_by_field_name(FUNCTION_FIELD)
        }
        _ => None,
    }
    .map(without_parentheses)
    .filter(|callee| callee.kind() == IDENTIFIER)?;
    let name = text(callee, context.source);
    (BUILT_IN_ERRORS.contains(&name) && constants.is_global(callee)).then_some(name)
}

// `getErrorCause`: el `cause` del objeto de opciones, si se puede saber cuál es.
fn error_cause<'tree>(
    thrown: Node<'tree>,
    options_index: usize,
    context: &Context<'tree, '_>,
) -> Cause<'tree> {
    let arguments = thrown
        .child_by_field_name(ARGUMENTS_FIELD)
        .map(values)
        .unwrap_or_default();
    if arguments
        .iter()
        .position(|argument| argument.kind() == SPREAD)
        .is_some_and(|spread| spread <= options_index)
    {
        return Cause::Unknown;
    }
    let Some(options) = arguments
        .get(options_index)
        .map(|options| without_parentheses(*options))
    else {
        return Cause::Missing;
    };
    if options.kind() != OBJECT {
        return Cause::Unknown;
    }
    let properties = values(options);
    if properties.iter().any(|property| property.kind() == SPREAD) {
        return Cause::Unknown;
    }
    properties
        .into_iter()
        .filter(|property| property_name(*property, context).is_some_and(|name| name == CAUSE))
        .last()
        .and_then(property_value)
        .map_or(Cause::Missing, Cause::Given)
}

fn property_name(property: Node<'_>, context: &Context<'_, '_>) -> Option<String> {
    match property.kind() {
        SHORTHAND => Some(context.text(property).to_owned()),
        PAIR => property
            .child_by_field_name(KEY_FIELD)
            .and_then(|key| static_key_name(key, context.source)),
        METHOD => property
            .child_by_field_name(NAME_FIELD)
            .and_then(|key| static_key_name(key, context.source)),
        _ => None,
    }
}

fn property_value(property: Node<'_>) -> Option<Node<'_>> {
    match property.kind() {
        SHORTHAND => Some(property),
        PAIR => property.child_by_field_name(VALUE_FIELD),
        METHOD => property.child_by_field_name(PARAMETERS_FIELD),
        _ => None,
    }
}

fn values(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .filter(|child| child.kind() != COMMENT)
        .collect()
}

// El error del `catch` quedó tapado por otra declaración con el mismo nombre más adentro.
fn is_shadowed(throw: Node<'_>, catch: Node<'_>, name: &str, context: &Context<'_, '_>) -> bool {
    let tree = context.scopes();
    let mut current = Some(tree.scope_of(throw));
    while let Some(scope) = current.and_then(|id| tree.scope(id)) {
        if scope.lookup(name).is_some() {
            return scope.block.id() != catch.id();
        }
        current = scope.upper;
    }
    true
}
