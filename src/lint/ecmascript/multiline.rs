use std::sync::LazyLock;

use regex::Regex;
use tree_sitter::Node;

use super::nodes::{ARGUMENTS, BINARY, CALL, FUNCTION_FIELD, LEFT_FIELD, OBJECT_FIELD, SUBSCRIPT, operator};
use super::token_index::TokenIndex;
use super::{Context, report};
use crate::lint::LintFinding;
use crate::rules::EslintRule;

const ARGUMENTS_FIELD: &str = "arguments";
const OPTIONAL_CHAIN_FIELD: &str = "optional_chain";
const TEMPLATE_STRING: &str = "template_string";
const CLOSING_PARENTHESIS: &str = ")";
const DIVISION: &str = "/";
// Una flecha sin paréntesis no puede ser lo que se llama: si tree-sitter lo lee así, JavaScript
// cerró la sentencia antes con el punto y coma automático.
const ARROW_FUNCTION: &str = "arrow_function";

static REGEX_FLAGS: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new("^[dgimsuvy]+$").ok());

// `no-unexpected-multiline`: una línea que JavaScript une con la anterior aunque se lea separada
// (`a\n(b)` es una llamada, `a\n[b]` un acceso, `` a\n`b` `` un template con etiqueta).
pub fn unexpected_multilines(context: &Context<'_, '_>) -> Vec<LintFinding> {
    let tokens = TokenIndex::new(context.root);
    let mut found = Vec::new();
    for node in context.of_kind(&[CALL, SUBSCRIPT, BINARY]) {
        let broken = match node.kind() {
            CALL => broken_call(node, &tokens),
            SUBSCRIPT if node.child_by_field_name(OPTIONAL_CHAIN_FIELD).is_none() => node
                .child_by_field_name(OBJECT_FIELD)
                .and_then(|object| break_after(object, &tokens)),
            _ => broken_division(node, &tokens, context),
        };
        found.extend(broken.map(|token| report(EslintRule::NoUnexpectedMultiline, token, "")));
    }
    found
}

fn broken_call<'tree>(call: Node<'tree>, tokens: &TokenIndex<'tree>) -> Option<Node<'tree>> {
    let callee = call
        .child_by_field_name(FUNCTION_FIELD)
        .filter(|callee| callee.kind() != ARROW_FUNCTION)?;
    let arguments = call.child_by_field_name(ARGUMENTS_FIELD)?;
    if arguments.kind() == TEMPLATE_STRING {
        let before = tokens.token_before(arguments.start_byte())?;
        return (before.end_position().row != arguments.start_position().row).then_some(arguments);
    }
    let has_arguments = arguments.kind() == ARGUMENTS && arguments.named_child_count() > 0;
    if !has_arguments || call.child_by_field_name(OPTIONAL_CHAIN_FIELD).is_some() {
        return None;
    }
    break_after(callee, tokens)
}

// `checkForBreakAfter`: el primer token después de la expresión (salteando sus `)`) empieza en otra
// línea que el anterior a él.
fn break_after<'tree>(expression: Node<'tree>, tokens: &TokenIndex<'tree>) -> Option<Node<'tree>> {
    let opening = tokens
        .tokens_after(expression.end_byte())
        .find(|token| token.kind() != CLOSING_PARENTHESIS)?;
    let before = tokens.token_before(opening.start_byte())?;
    (opening.start_position().row != before.end_position().row).then_some(opening)
}

// `a\n/foo/g`: lo que parece una expresión regular en otra línea es una división de la anterior.
fn broken_division<'tree>(
    outer: Node<'tree>,
    tokens: &TokenIndex<'tree>,
    context: &Context<'tree, '_>,
) -> Option<Node<'tree>> {
    if operator(outer, context.source) != DIVISION {
        return None;
    }
    let inner = outer
        .child_by_field_name(LEFT_FIELD)
        .filter(|left| left.kind() == BINARY && operator(*left, context.source) == DIVISION)?;
    let mut after = tokens.tokens_after(inner.end_byte());
    let second_slash = after.find(|token| context.text(*token) == DIVISION)?;
    let flags = after.next()?;
    let looks_like_flags = flags.kind() == "identifier"
        && REGEX_FLAGS
            .as_ref()
            .is_some_and(|pattern| pattern.is_match(context.text(flags)))
        && second_slash.end_byte() == flags.start_byte();
    if !looks_like_flags {
        return None;
    }
    inner
        .child_by_field_name(LEFT_FIELD)
        .and_then(|left| break_after(left, tokens))
}
