use tree_sitter::Node;

use super::constants::Constants;
use super::nodes::{BODY_FIELD, COMMENT, IDENTIFIER, first_named_child, text, without_parentheses};
use super::{Context, report};
use crate::lint::LintFinding;
use crate::rules::EslintRule;

const CATCH: &str = "catch_clause";
const PARAMETER_FIELD: &str = "parameter";
const FINALIZER_FIELD: &str = "finalizer";
const THROW: &str = "throw_statement";
const LABELED: &str = "labeled_statement";
const LABEL_FIELD: &str = "label";
const JUMPS: &[&str] = &["break_statement", "continue_statement"];
const YIELD: &str = "yield_expression";
const GENERATOR_STAR: &str = "*";
const GENERATORS: &[&str] = &[
    "generator_function_declaration",
    "generator_function",
    "method_definition",
];
const HEAD_OWNERS: &[&str] = &[
    "pair",
    "method_definition",
    "public_field_definition",
    "field_definition",
];
const NESTED_FUNCTIONS: &[&str] = &[
    "function_declaration",
    "function_expression",
    "function",
    "generator_function_declaration",
    "generator_function",
    "method_definition",
    "arrow_function",
];
const NEW: &str = "new_expression";
const CONSTRUCTOR_FIELD: &str = "constructor";
const ARGUMENTS_FIELD: &str = "arguments";
const PROMISE: &str = "Promise";
const ASYNC: &str = "async";
const EXECUTORS: &[&str] = &["arrow_function", "function_expression", "function"];

// `no-useless-catch`: un `catch (e) { throw e; ... }` que sólo vuelve a tirar lo que agarró.
pub fn useless_catches(context: &Context<'_, '_>) -> Vec<LintFinding> {
    context
        .of_kind(&[CATCH])
        .filter(|catch| rethrows_parameter(*catch, context.source))
        .filter_map(|catch| {
            let statement = catch.parent()?;
            let has_finalizer = statement.child_by_field_name(FINALIZER_FIELD).is_some();
            Some(report(
                EslintRule::NoUselessCatch,
                if has_finalizer { catch } else { statement },
                "",
            ))
        })
        .collect()
}

fn rethrows_parameter(catch: Node<'_>, source: &[u8]) -> bool {
    let Some(parameter) = catch
        .child_by_field_name(PARAMETER_FIELD)
        .filter(|parameter| parameter.kind() == IDENTIFIER)
    else {
        return false;
    };
    catch
        .child_by_field_name(BODY_FIELD)
        .and_then(first_named_child)
        .filter(|statement| statement.kind() == THROW)
        .and_then(first_named_child)
        .map(without_parentheses)
        .is_some_and(|thrown| thrown.kind() == IDENTIFIER && text(thrown, source) == text(parameter, source))
}

// `no-unused-labels`: una etiqueta que ningún `break` ni `continue` usa.
pub fn unused_labels(context: &Context<'_, '_>) -> Vec<LintFinding> {
    context
        .of_kind(&[LABELED])
        .filter_map(|statement| statement.child_by_field_name(LABEL_FIELD))
        .filter(|label| !is_label_used(*label, context.source))
        .map(|label| report(EslintRule::NoUnusedLabels, label, context.text(label)))
        .collect()
}

fn is_label_used(label: Node<'_>, source: &[u8]) -> bool {
    let name = text(label, source);
    let Some(body) = label
        .parent()
        .and_then(|statement| statement.child_by_field_name(BODY_FIELD))
    else {
        return false;
    };
    let mut pending = vec![body];
    while let Some(current) = pending.pop() {
        if JUMPS.contains(&current.kind())
            && current
                .child_by_field_name(LABEL_FIELD)
                .is_some_and(|target| text(target, source) == name)
        {
            return true;
        }
        let mut cursor = current.walk();
        pending.extend(current.named_children(&mut cursor));
    }
    false
}

// `require-yield`: un generador con cuerpo y sin ningún `yield` propio.
pub fn generators_without_yield(context: &Context<'_, '_>) -> Vec<LintFinding> {
    context
        .of_kind(GENERATORS)
        .filter(|function| is_generator(*function))
        .filter(|function| {
            function
                .child_by_field_name(BODY_FIELD)
                .is_some_and(|body| has_statements(body) && !yields(body))
        })
        .map(|function| report(EslintRule::RequireYield, head_start(function), ""))
        .collect()
}

fn is_generator(function: Node<'_>) -> bool {
    if function.kind() != "method_definition" {
        return true;
    }
    let mut cursor = function.walk();
    function
        .children(&mut cursor)
        .any(|child| child.kind() == GENERATOR_STAR)
}

fn has_statements(body: Node<'_>) -> bool {
    let mut cursor = body.walk();
    body.named_children(&mut cursor)
        .any(|child| child.kind() != COMMENT)
}

fn yields(body: Node<'_>) -> bool {
    let mut pending = vec![body];
    while let Some(current) = pending.pop() {
        if current.kind() == YIELD {
            return true;
        }
        let mut cursor = current.walk();
        pending.extend(
            current
                .named_children(&mut cursor)
                .filter(|child| !NESTED_FUNCTIONS.contains(&child.kind())),
        );
    }
    false
}

fn head_start(function: Node<'_>) -> Node<'_> {
    function
        .parent()
        .filter(|parent| HEAD_OWNERS.contains(&parent.kind()) && function.kind() != "method_definition")
        .unwrap_or(function)
}

// `no-async-promise-executor`: `new Promise(async (resolve) => ...)`, que se traga los errores.
pub fn async_promise_executors(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let constants = Constants::new(context);
    context
        .of_kind(&[NEW])
        .filter(|new| {
            new.child_by_field_name(CONSTRUCTOR_FIELD)
                .map(without_parentheses)
                .is_some_and(|constructor| {
                    constructor.kind() == IDENTIFIER
                        && context.text(constructor) == PROMISE
                        && constants.is_global(constructor)
                })
        })
        .filter_map(|new| {
            let executor = new
                .child_by_field_name(ARGUMENTS_FIELD)
                .and_then(first_named_child)
                .map(without_parentheses)
                .filter(|executor| EXECUTORS.contains(&executor.kind()))?;
            let mut cursor = executor.walk();
            executor.children(&mut cursor).find(|child| child.kind() == ASYNC)
        })
        .map(|keyword| report(EslintRule::NoAsyncPromiseExecutor, keyword, ASYNC))
        .collect()
}
