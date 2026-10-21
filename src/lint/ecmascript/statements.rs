use tree_sitter::Node;

use super::nodes::{
    ARGUMENTS, ARGUMENTS_FIELD, BODY_FIELD, CALL, COMMENT, CONDITION_FIELD, FUNCTION_FIELD, INDEX_FIELD,
    MEMBER, PROPERTY_FIELD, STATEMENT_BLOCK, SUBSCRIPT, SWITCH_CASE, SWITCH_DEFAULT, enclosed_children,
    field_children, first_named_child, is_assignment, is_function, parenthesis_layers, without_parentheses,
};
use super::strings::static_string;
use super::{Context, report, report_with_fix};
use crate::fix::Fix;
use crate::lint::LintFinding;
use crate::rules::EslintRule;

const DEBUGGER: &str = "debugger_statement";
const STATEMENT_LISTS: &[&str] = &["program", STATEMENT_BLOCK, SWITCH_CASE, SWITCH_DEFAULT];
const NOT_BLOCK_STATEMENT_PARENTS: &[&str] = &[
    "class_static_block",
    "internal_module",
    "module",
    "ambient_declaration",
];
const SWITCH: &str = "switch_statement";
const SWITCH_BODY: &str = "switch_body";
const CASE_KINDS: &[&str] = &[SWITCH_CASE, SWITCH_DEFAULT];
const LEXICAL_DECLARATION: &str = "lexical_declaration";
const CASE_DECLARATIONS: &[&str] = &[
    LEXICAL_DECLARATION,
    "function_declaration",
    "generator_function_declaration",
    "class_declaration",
    "abstract_class_declaration",
];
const STATEMENT_TESTS: &[&str] = &["if_statement", "while_statement", "do_statement"];
const TERNARY: &str = "ternary_expression";
const FOR: &str = "for_statement";
const EMPTY_STATEMENT: &str = "empty_statement";
const SEMICOLON: &str = ";";
const TWICE: usize = 2;
const PROTOTYPE_BUILTINS: &[&str] = &["hasOwnProperty", "isPrototypeOf", "propertyIsEnumerable"];

pub fn debugger(context: &Context<'_, '_>) -> Vec<LintFinding> {
    context
        .of_kind(&[DEBUGGER])
        .map(|statement| {
            let removable = statement
                .parent()
                .is_some_and(|parent| STATEMENT_LISTS.contains(&parent.kind()));
            if removable {
                report_with_fix(
                    EslintRule::NoDebugger,
                    statement,
                    "",
                    Fix::remove(statement, context.source),
                )
            } else {
                report(EslintRule::NoDebugger, statement, "")
            }
        })
        .collect()
}

pub fn empty_blocks(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let blocks = context
        .of_kind(&[STATEMENT_BLOCK])
        .filter(|block| enclosed_children(*block).is_empty() && is_block_statement(*block))
        .map(|block| report(EslintRule::NoEmpty, block, ""));
    let switches = context
        .of_kind(&[SWITCH])
        .filter_map(|switch| switch.child_by_field_name(BODY_FIELD))
        .filter(|body| body.kind() == SWITCH_BODY && enclosed_children(*body).is_empty())
        .map(|body| report(EslintRule::NoEmpty, body, ""));
    blocks.chain(switches).collect()
}

fn is_block_statement(block: Node<'_>) -> bool {
    block
        .parent()
        .is_none_or(|parent| !is_function(parent) && !NOT_BLOCK_STATEMENT_PARENTS.contains(&parent.kind()))
}

pub fn case_declarations(context: &Context<'_, '_>) -> Vec<LintFinding> {
    context
        .of_kind(CASE_KINDS)
        .flat_map(|case| field_children(case, BODY_FIELD))
        .filter(|statement| is_lexical_declaration(*statement, context))
        .map(|statement| report(EslintRule::NoCaseDeclarations, statement, ""))
        .collect()
}

fn is_lexical_declaration(statement: Node<'_>, context: &Context<'_, '_>) -> bool {
    CASE_DECLARATIONS.contains(&statement.kind())
        && (statement.kind() != LEXICAL_DECLARATION
            || statement
                .child_by_field_name("kind")
                .is_some_and(|kind| context.text(kind) != "var"))
}

pub fn conditional_assignments(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let statements = context
        .of_kind(STATEMENT_TESTS)
        .filter_map(|statement| statement.child_by_field_name(CONDITION_FIELD))
        .filter_map(|condition| first_named_child(condition))
        .filter(|test| parenthesis_layers(*test) < 1);
    let ternaries = context
        .of_kind(&[TERNARY])
        .filter_map(|ternary| ternary.child_by_field_name(CONDITION_FIELD))
        .filter(|test| parenthesis_layers(*test) < TWICE);
    let loops = context
        .of_kind(&[FOR])
        .filter_map(for_test)
        .filter(|test| parenthesis_layers(*test) == 0);
    statements
        .chain(ternaries)
        .chain(loops)
        .map(without_parentheses)
        .filter(|test| is_assignment(*test))
        .map(|test| report(EslintRule::NoCondAssign, test, ""))
        .collect()
}

fn for_test(statement: Node<'_>) -> Option<Node<'_>> {
    field_children(statement, CONDITION_FIELD)
        .into_iter()
        .find(|condition| condition.is_named() && ![EMPTY_STATEMENT, COMMENT].contains(&condition.kind()))
        .filter(|condition| condition.kind() != SEMICOLON)
}

pub fn prototype_builtins(context: &Context<'_, '_>) -> Vec<LintFinding> {
    context
        .of_kind(&[CALL])
        .filter(|call| {
            call.child_by_field_name(ARGUMENTS_FIELD)
                .is_some_and(|arguments| arguments.kind() == ARGUMENTS)
        })
        .filter_map(|call| call.child_by_field_name(FUNCTION_FIELD).map(without_parentheses))
        .filter_map(|callee| static_property(callee, context))
        .filter(|(_, name)| PROTOTYPE_BUILTINS.contains(&name.as_str()))
        .map(|(property, name)| report(EslintRule::NoPrototypeBuiltins, property, &name))
        .collect()
}

fn static_property<'tree>(
    callee: Node<'tree>,
    context: &Context<'tree, '_>,
) -> Option<(Node<'tree>, String)> {
    match callee.kind() {
        MEMBER => {
            let property = callee.child_by_field_name(PROPERTY_FIELD)?;
            (property.kind() == "property_identifier").then(|| (property, context.text(property).to_owned()))
        }
        SUBSCRIPT => {
            let index = callee.child_by_field_name(INDEX_FIELD)?;
            let index = without_parentheses(index);
            static_string(index, context.source).map(|name| (index, name))
        }
        _ => None,
    }
}
