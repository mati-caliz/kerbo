use tree_sitter::Node;

use super::constants::Constants;
use super::members::{static_key_name, static_property_name};
use super::nodes::{
    ARGUMENTS, BODY_FIELD, CALL, COMMENT, FUNCTION_FIELD, IDENTIFIER, KEY_FIELD, MEMBER, NAME_FIELD,
    OBJECT_FIELD, STATEMENT_BLOCK, SUBSCRIPT, VALUE_FIELD, parent_without_parentheses, without_parentheses,
};
use super::{Context, report};
use crate::lint::LintFinding;
use crate::rules::EslintRule;

const METHOD: &str = "method_definition";
const PAIR: &str = "pair";
const OBJECT: &str = "object";
const SET: &str = "set";
const RETURN: &str = "return_statement";
const ARROW: &str = "arrow_function";
const FUNCTION_VALUES: &[&str] = &["function_expression", "function", ARROW];
const NESTED_FUNCTIONS: &[&str] = &[
    "function_declaration",
    "function_expression",
    "function",
    "generator_function_declaration",
    "generator_function",
    METHOD,
    ARROW,
];
const OBJECT_GLOBAL: &str = "Object";
const REFLECT_GLOBAL: &str = "Reflect";
const DESCRIPTOR_CALLS: &[(&str, &str, usize)] = &[
    (OBJECT_GLOBAL, "defineProperty", 2),
    (REFLECT_GLOBAL, "defineProperty", 2),
];
const DESCRIPTOR_MAP_CALLS: &[(&str, &str, usize)] = &[
    (OBJECT_GLOBAL, "create", 1),
    (OBJECT_GLOBAL, "defineProperties", 1),
];

// `no-setter-return`: un setter que devuelve un valor, que nadie puede leer.
pub fn setter_returns(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let constants = Constants::new(context);
    let mut found = Vec::new();
    for setter in context
        .of_kind(&[METHOD])
        .filter(|method| is_setter_method(*method))
    {
        if let Some(body) = setter.child_by_field_name(BODY_FIELD) {
            found.extend(returns_with_value(body));
        }
    }
    for method in context
        .of_kind(&[METHOD])
        .filter(|method| is_descriptor_set_method(*method, &constants, context))
    {
        if let Some(body) = method.child_by_field_name(BODY_FIELD) {
            found.extend(returns_with_value(body));
        }
    }
    for function in context
        .of_kind(FUNCTION_VALUES)
        .filter(|function| is_descriptor_setter(*function, &constants, context))
    {
        match function.child_by_field_name(BODY_FIELD) {
            Some(body) if body.kind() == STATEMENT_BLOCK => found.extend(returns_with_value(body)),
            Some(body) => found.push(body),
            None => {}
        }
    }
    found
        .into_iter()
        .map(|node| report(EslintRule::NoSetterReturn, node, ""))
        .collect()
}

fn is_setter_method(method: Node<'_>) -> bool {
    let mut cursor = method.walk();
    method
        .children(&mut cursor)
        .any(|child| child.kind() == SET && !child.is_named())
}

fn is_descriptor_set_method(
    method: Node<'_>,
    constants: &Constants<'_, '_, '_>,
    context: &Context<'_, '_>,
) -> bool {
    !is_setter_method(method)
        && method
            .child_by_field_name(NAME_FIELD)
            .and_then(|name| static_key_name(name, context.source))
            .is_some_and(|name| name == SET)
        && method
            .parent()
            .filter(|object| object.kind() == OBJECT)
            .is_some_and(|object| is_property_descriptor(object, constants, context))
}

fn is_descriptor_setter(
    function: Node<'_>,
    constants: &Constants<'_, '_, '_>,
    context: &Context<'_, '_>,
) -> bool {
    let Some(pair) = parent_without_parentheses(function).filter(|parent| parent.kind() == PAIR) else {
        return false;
    };
    let is_value = pair
        .child_by_field_name(VALUE_FIELD)
        .map(without_parentheses)
        .is_some_and(|value| value.id() == function.id());
    let named_set = pair
        .child_by_field_name(KEY_FIELD)
        .and_then(|key| static_key_name(key, context.source))
        .is_some_and(|name| name == SET);
    is_value
        && named_set
        && pair
            .parent()
            .filter(|object| object.kind() == OBJECT)
            .is_some_and(|object| is_property_descriptor(object, constants, context))
}

// `isPropertyDescriptor`: el objeto que reciben `Object.defineProperty` y compañía.
fn is_property_descriptor(
    object: Node<'_>,
    constants: &Constants<'_, '_, '_>,
    context: &Context<'_, '_>,
) -> bool {
    if DESCRIPTOR_CALLS
        .iter()
        .any(|call| is_global_method_argument(object, *call, constants, context))
    {
        return true;
    }
    let Some(pair) = parent_without_parentheses(object).filter(|parent| parent.kind() == PAIR) else {
        return false;
    };
    let is_value = pair
        .child_by_field_name(VALUE_FIELD)
        .map(without_parentheses)
        .is_some_and(|value| value.id() == object.id());
    is_value
        && pair
            .parent()
            .filter(|map| map.kind() == OBJECT)
            .is_some_and(|map| {
                DESCRIPTOR_MAP_CALLS
                    .iter()
                    .any(|call| is_global_method_argument(map, *call, constants, context))
            })
}

pub fn is_global_method_argument(
    node: Node<'_>,
    (object_name, method_name, index): (&str, &str, usize),
    constants: &Constants<'_, '_, '_>,
    context: &Context<'_, '_>,
) -> bool {
    let Some(arguments) = parent_without_parentheses(node).filter(|parent| parent.kind() == ARGUMENTS) else {
        return false;
    };
    let at_index = argument_values(arguments)
        .get(index)
        .map(|argument| without_parentheses(*argument))
        .is_some_and(|argument| argument.id() == node.id());
    let Some(callee) = arguments
        .parent()
        .filter(|call| call.kind() == CALL)
        .and_then(|call| call.child_by_field_name(FUNCTION_FIELD))
        .map(without_parentheses)
        .filter(|callee| [MEMBER, SUBSCRIPT].contains(&callee.kind()))
    else {
        return false;
    };
    at_index
        && static_property_name(callee, context.source).is_some_and(|name| name == method_name)
        && callee
            .child_by_field_name(OBJECT_FIELD)
            .map(without_parentheses)
            .is_some_and(|object| {
                object.kind() == IDENTIFIER
                    && context.text(object) == object_name
                    && constants.is_global(object)
            })
}

fn argument_values(arguments: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = arguments.walk();
    arguments
        .named_children(&mut cursor)
        .filter(|child| child.kind() != COMMENT)
        .collect()
}

fn returns_with_value(body: Node<'_>) -> Vec<Node<'_>> {
    let mut found = Vec::new();
    let mut pending = vec![body];
    while let Some(current) = pending.pop() {
        if current.kind() == RETURN
            && current
                .named_child(0)
                .is_some_and(|child| child.kind() != COMMENT)
        {
            found.push(current);
        }
        let mut cursor = current.walk();
        pending.extend(
            current
                .named_children(&mut cursor)
                .filter(|child| !NESTED_FUNCTIONS.contains(&child.kind())),
        );
    }
    found
}
