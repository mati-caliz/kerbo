use std::collections::HashSet;

use tree_sitter::Node;

use super::constants::Constants;
use super::escapes::point_at;
use super::members::static_key_name;
use super::nodes::{
    CALL, FUNCTION_FIELD, IDENTIFIER, LEFT_FIELD, MEMBER, NAME_FIELD, OBJECT_FIELD, PROPERTY_FIELD, STRING,
    operator, text, without_parentheses,
};
use super::scope::DefinitionKind;
use super::{Context, report};
use crate::lint::{LintFinding, LintKind, finding_at};
use crate::rules::EslintRule;

const UNARY: &str = "unary_expression";
const ARGUMENT_FIELD: &str = "argument";
const DELETE: &str = "delete";
const NEGATION: &str = "!";
const IN_OPERATOR: &str = "in";
const INSTANCEOF_OPERATOR: &str = "instanceof";
const BINARY: &str = "binary_expression";
const WITH: &str = "with_statement";
const NUMBER: &str = "number";
const STATIC_BLOCK: &str = "class_static_block";
const BODY_FIELD: &str = "body";
const NEW: &str = "new_expression";
const CONSTRUCTOR_FIELD: &str = "constructor";
const NON_CONSTRUCTORS: &[&str] = &["Symbol", "BigInt"];
const NON_CALLABLE_GLOBALS: &[&str] = &[
    "Atomics",
    "JSON",
    "Math",
    "Reflect",
    "Intl",
    "Temporal",
    "globalThis",
];
const GLOBAL_THIS: &str = "globalThis";
const PARAMETERS_FIELD: &str = "parameters";
const NAMED_FUNCTIONS: &[&str] = &[
    "function_declaration",
    "function_expression",
    "function",
    "generator_function_declaration",
    "generator_function",
];
const METHOD: &str = "method_definition";
const CLASS_BODY: &str = "class_body";
const FIELDS: &[&str] = &["field_definition", "public_field_definition"];
const STATIC: &str = "static";
const GET: &str = "get";
const SET: &str = "set";
const CONSTRUCTOR_NAME: &str = "constructor";
const OCTAL_START: char = '0';
const BACKSLASH: u8 = b'\\';
const NON_OCTAL_DIGITS: &[u8] = b"89";
const ESCAPE_LENGTH: usize = 2;

fn unary_operator<'source>(node: Node<'_>, source: &'source [u8]) -> &'source str {
    operator(node, source)
}

// `no-delete-var`.
pub fn deleted_variables(context: &Context<'_, '_>) -> Vec<LintFinding> {
    context
        .of_kind(&[UNARY])
        .filter(|unary| unary_operator(*unary, context.source) == DELETE)
        .filter(|unary| {
            unary
                .child_by_field_name(ARGUMENT_FIELD)
                .is_some_and(|argument| without_parentheses(argument).kind() == IDENTIFIER)
        })
        .map(|unary| report(EslintRule::NoDeleteVar, unary, context.text(unary)))
        .collect()
}

// `no-with`.
pub fn with_statements(context: &Context<'_, '_>) -> Vec<LintFinding> {
    context
        .of_kind(&[WITH])
        .map(|statement| report(EslintRule::NoWith, statement, ""))
        .collect()
}

// `no-unsafe-negation`: `!a in b`.
pub fn unsafe_negations(context: &Context<'_, '_>) -> Vec<LintFinding> {
    context
        .of_kind(&[BINARY])
        .filter(|binary| [IN_OPERATOR, INSTANCEOF_OPERATOR].contains(&operator(*binary, context.source)))
        .filter_map(|binary| binary.child_by_field_name(LEFT_FIELD))
        .filter(|left| left.kind() == UNARY && unary_operator(*left, context.source) == NEGATION)
        .map(|left| report(EslintRule::NoUnsafeNegation, left, context.text(left)))
        .collect()
}

// `no-octal`: la expresión `^0\d`.
pub fn octal_literals(context: &Context<'_, '_>) -> Vec<LintFinding> {
    context
        .of_kind(&[NUMBER])
        .filter(|number| {
            let mut characters = context.text(*number).chars();
            characters.next() == Some(OCTAL_START)
                && characters.next().is_some_and(|digit| digit.is_ascii_digit())
        })
        .map(|number| report(EslintRule::NoOctal, number, context.text(number)))
        .collect()
}

// `no-nonoctal-decimal-escape`: `\8` y `\9`, que no son octales ni un escape válido.
pub fn nonoctal_escapes(context: &Context<'_, '_>) -> Vec<LintFinding> {
    context
        .of_kind(&[STRING])
        .flat_map(|string| {
            let raw = context.text(string).as_bytes();
            let mut found = Vec::new();
            let mut index = 0;
            while let Some(byte) = raw.get(index) {
                if *byte != BACKSLASH {
                    index = index.saturating_add(1);
                    continue;
                }
                if raw
                    .get(index.saturating_add(1))
                    .is_some_and(|next| NON_OCTAL_DIGITS.contains(next))
                {
                    found.push(finding_at(
                        LintKind::Eslint(EslintRule::NoNonoctalDecimalEscape),
                        point_at(string, index, context.source),
                        "",
                        None,
                    ));
                }
                index = index.saturating_add(ESCAPE_LENGTH);
            }
            found
        })
        .collect()
}

// `no-empty-static-block`: los comentarios adentro cuentan como contenido.
pub fn empty_static_blocks(context: &Context<'_, '_>) -> Vec<LintFinding> {
    context
        .of_kind(&[STATIC_BLOCK])
        .filter_map(|block| block.child_by_field_name(BODY_FIELD))
        .filter(|body| body.named_child_count() == 0)
        .map(|body| report(EslintRule::NoEmptyStaticBlock, body, ""))
        .collect()
}

// `no-new-native-nonconstructor`.
pub fn new_native_nonconstructors(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let constants = Constants::new(context);
    context
        .of_kind(&[NEW])
        .filter_map(|new| new.child_by_field_name(CONSTRUCTOR_FIELD))
        .filter(|callee| {
            callee.kind() == IDENTIFIER
                && NON_CONSTRUCTORS.contains(&context.text(*callee))
                && constants.is_global(*callee)
        })
        .map(|callee| {
            report(
                EslintRule::NoNewNativeNonconstructor,
                callee,
                context.text(callee),
            )
        })
        .collect()
}

// `no-obj-calls`: `Math()`, `new JSON()` y `globalThis.Reflect()`.
pub fn object_calls(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let constants = Constants::new(context);
    context
        .of_kind(&[CALL, NEW])
        .filter(|call| {
            call.child_by_field_name(FUNCTION_FIELD)
                .or_else(|| call.child_by_field_name(CONSTRUCTOR_FIELD))
                .is_some_and(|callee| is_non_callable(without_parentheses(callee), &constants, context))
        })
        .map(|call| report(EslintRule::NoObjCalls, call, ""))
        .collect()
}

fn is_non_callable(callee: Node<'_>, constants: &Constants<'_, '_, '_>, context: &Context<'_, '_>) -> bool {
    if callee.kind() == IDENTIFIER {
        return NON_CALLABLE_GLOBALS.contains(&context.text(callee)) && constants.is_global(callee);
    }
    callee.kind() == MEMBER
        && callee.child_by_field_name(OBJECT_FIELD).is_some_and(|object| {
            object.kind() == IDENTIFIER && context.text(object) == GLOBAL_THIS && constants.is_global(object)
        })
        && callee
            .child_by_field_name(PROPERTY_FIELD)
            .is_some_and(|property| NON_CALLABLE_GLOBALS.contains(&context.text(property)))
}

// `no-dupe-args`.
pub fn duplicate_arguments(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let tree = context.scopes();
    let mut kinds = NAMED_FUNCTIONS.to_vec();
    kinds.push(METHOD);
    context
        .nodes
        .iter()
        .copied()
        .filter(|node| kinds.contains(&node.kind()))
        .flat_map(|function| {
            let holder = function.child_by_field_name(PARAMETERS_FIELD).unwrap_or(function);
            tree.declared_by(function)
                .iter()
                .filter_map(|id| tree.variable(*id))
                .filter(|variable| {
                    variable
                        .definitions
                        .iter()
                        .filter(|definition| matches!(definition.kind, DefinitionKind::Parameter))
                        .count()
                        > 1
                })
                .map(|variable| report(EslintRule::NoDupeArgs, holder, &variable.name))
                .collect::<Vec<_>>()
        })
        .collect()
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum MemberKind {
    Init,
    Get,
    Set,
}

// `no-dupe-class-members`.
pub fn duplicate_class_members(context: &Context<'_, '_>) -> Vec<LintFinding> {
    context
        .of_kind(&[CLASS_BODY])
        .flat_map(|body| duplicates_in_body(body, context))
        .collect()
}

fn has_token(member: Node<'_>, token: &str, source: &[u8]) -> bool {
    (0..member.child_count())
        .filter_map(|index| member.child(index))
        .any(|child| !child.is_named() && text(child, source) == token)
}

fn duplicates_in_body(body: Node<'_>, context: &Context<'_, '_>) -> Vec<LintFinding> {
    let source = context.source;
    let mut seen: HashSet<(String, bool, MemberKind)> = HashSet::new();
    let mut found = Vec::new();
    let mut cursor = body.walk();
    for member in body.named_children(&mut cursor) {
        let is_method = member.kind() == METHOD;
        if !is_method && !FIELDS.contains(&member.kind()) {
            continue;
        }
        let Some(key) = member
            .child_by_field_name(NAME_FIELD)
            .or_else(|| member.child_by_field_name(PROPERTY_FIELD))
        else {
            continue;
        };
        let Some(name) = static_key_name(key, source) else {
            continue;
        };
        let is_static = has_token(member, STATIC, source);
        if is_method && name == CONSTRUCTOR_NAME && !is_static {
            continue;
        }
        let kind = if is_method && has_token(member, GET, source) {
            MemberKind::Get
        } else if is_method && has_token(member, SET, source) {
            MemberKind::Set
        } else {
            MemberKind::Init
        };
        let clashes: &[MemberKind] = match kind {
            MemberKind::Get => &[MemberKind::Init, MemberKind::Get],
            MemberKind::Set => &[MemberKind::Init, MemberKind::Set],
            MemberKind::Init => &[MemberKind::Init, MemberKind::Get, MemberKind::Set],
        };
        let is_duplicate = clashes
            .iter()
            .any(|clash| seen.contains(&(name.clone(), is_static, *clash)));
        seen.insert((name.clone(), is_static, kind));
        if is_duplicate {
            found.push(report(EslintRule::NoDupeClassMembers, key, &name));
        }
    }
    found
}
