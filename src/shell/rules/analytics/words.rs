use tree_sitter::Node;

use crate::rules::ShellRule;
use crate::shell::commands::Command;
use crate::shell::context::{Context, Findings};
use crate::shell::parameters::{is_array_expansion, is_variable_character, modifier};
use crate::shell::quoting::{Strictness, is_quote_free};
use crate::shell::rules::command_checks::word_shapes::{is_glob, will_split};
use crate::shell::syntax::{
    ARITHMETIC_EXPANSION, ASSIGNMENT, BINARY, COMMAND, COMMAND_SUBSTITUTION, CONCATENATION, DOUBLE_BRACKET,
    EXPANSIONS, FOR, LEFT_FIELD, NAME_FIELD, OPERATOR_FIELD, PIPELINE, RAW_STRING, STRING, STRING_CONTENT,
    TEST, UNARY, VALUE_FIELD, children, first_token, named_children, text,
};
use crate::shell::words::{braced_content, is_brace_expansion, parts};

const ALL_ARGUMENTS: &str = "*";
const ALL_ELEMENTS: &str = "[*]";
const HOME_PREFIX: &str = "~/";
const SELECT: &str = "select";
const LINE_BASED_COMMANDS: &[&str] = &["grep", "fgrep", "egrep", "sed", "cat", "awk", "cut", "sort"];
const PATH_SEPARATOR: char = '/';
const NON_EMPTY: &str = "-n";
const DOLLAR_PARENTHESIS: &str = "$(";
const BACKTICK: &str = "`";
const OLD_ARITHMETIC: &str = "$[";
const BRACE_EXPRESSION: &str = "brace_expression";

pub fn check(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let source = context.source;
    if EXPANSIONS.contains(&node.kind()) {
        check_dollar_star(node, context, findings);
    }
    match node.kind() {
        ASSIGNMENT => check_assignment(node, source, findings),
        FOR => check_for_in_lines(node, source, findings),
        UNARY => check_unquoted_non_empty(node, source, findings),
        COMMAND => check_executed_output(node, source, findings),
        ARITHMETIC_EXPANSION if first_token(node, source) == OLD_ARITHMETIC => {
            findings.report(ShellRule::Sc2007, node, OLD_ARITHMETIC);
        }
        _ => {}
    }
    if is_word(node) {
        check_concatenated_array(node, context, findings);
        check_tilde(node, source, findings);
    }
}

// Una palabra entera: un argumento, un valor o un destino, no una parte de otra palabra.
fn is_word(node: Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    let is_part = [CONCATENATION, STRING].contains(&parent.kind()) || EXPANSIONS.contains(&parent.kind());
    node.is_named() && !is_part && node.kind() != COMMAND && parent.kind() != PIPELINE
}

// SC2048: `$*` o `${arr[*]}` solos y sin comillas.
fn check_dollar_star(expansion: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let standalone = expansion.parent().is_some_and(|parent| {
        ![CONCATENATION, STRING].contains(&parent.kind()) && !EXPANSIONS.contains(&parent.kind())
    });
    if !standalone || is_quote_free(expansion, context, Strictness::Strict) {
        return;
    }
    let braced = braced_content(expansion, context.source);
    let array_star = modifier(&braced).starts_with(ALL_ELEMENTS)
        && braced.chars().next().is_some_and(is_variable_character);
    if braced.starts_with(ALL_ARGUMENTS) || array_star {
        findings.report(ShellRule::Sc2048, expansion, &braced);
    }
}

fn is_array_part(part: Node<'_>, source: &[u8]) -> bool {
    EXPANSIONS.contains(&part.kind()) && is_array_expansion(&braced_content(part, source))
}

// SC2145: un arreglo pegado a otro texto en la misma palabra.
fn check_concatenated_array(word: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let pieces = parts(word);
    if pieces.len() < 2 || is_quote_free(word, context, Strictness::Lenient) {
        return;
    }
    if let Some(array) = pieces
        .into_iter()
        .find(|part| is_array_part(*part, context.source))
    {
        findings.report(ShellRule::Sc2145, array, text(array, context.source));
    }
}

// SC2124 y SC2125: un arreglo o un glob asignado a una variable común.
fn check_assignment(assignment: Node<'_>, source: &[u8], findings: &mut Findings) {
    let Some(value) = assignment.child_by_field_name(VALUE_FIELD) else {
        return;
    };
    if parts(value).into_iter().any(|part| is_array_part(part, source)) {
        findings.report(ShellRule::Sc2124, value, text(value, source));
    } else if is_glob(value, source)
        || value.kind() == BRACE_EXPRESSION
        || (value.kind() == CONCATENATION && is_brace_expansion(value, source))
    {
        findings.report(ShellRule::Sc2125, value, text(value, source));
    }
}

// SC2088: `'~/x'` o `"~/x"`, donde la tilde no se expande.
fn check_tilde(word: Node<'_>, source: &[u8], findings: &mut Findings) {
    let first = if word.kind() == CONCATENATION {
        children(word).into_iter().next()
    } else {
        Some(word)
    };
    let Some(first) = first else {
        return;
    };
    let quoted = match first.kind() {
        RAW_STRING => Some(first).filter(|raw| {
            text(*raw, source)
                .get(1..)
                .is_some_and(|inner| inner.starts_with(HOME_PREFIX))
        }),
        STRING => named_children(first)
            .into_iter()
            .next()
            .filter(|part| part.kind() == STRING_CONTENT && text(*part, source).starts_with(HOME_PREFIX)),
        _ => None,
    };
    if let Some(quoted) = quoted {
        findings.report(ShellRule::Sc2088, quoted, text(quoted, source));
    }
}

// SC2013: `for x in $(cat archivo)`, que recorre palabras y no líneas.
fn check_for_in_lines(loop_node: Node<'_>, source: &[u8], findings: &mut Findings) {
    if first_token(loop_node, source) == SELECT {
        return;
    }
    let values: Vec<Node<'_>> = loop_node
        .children_by_field_name(VALUE_FIELD, &mut loop_node.walk())
        .collect();
    let [value] = values.as_slice() else {
        return;
    };
    let pieces = if value.kind() == CONCATENATION {
        children(*value)
    } else {
        vec![*value]
    };
    for substitution in pieces
        .into_iter()
        .filter(|piece| piece.kind() == COMMAND_SUBSTITUTION)
    {
        if reads_lines(substitution, source) {
            findings.report(ShellRule::Sc2013, substitution, text(substitution, source));
        }
    }
}

fn reads_lines(substitution: Node<'_>, source: &[u8]) -> bool {
    let [statement] = named_children(substitution)[..] else {
        return false;
    };
    let stages = if statement.kind() == PIPELINE {
        named_children(statement)
    } else {
        vec![statement]
    };
    stages.into_iter().all(|stage| {
        Command::of(stage)
            .and_then(|command| command.name(source))
            .is_some_and(|name| {
                LINE_BASED_COMMANDS.iter().any(|expected| {
                    name == *expected
                        || name
                            .strip_suffix(expected)
                            .is_some_and(|directory| directory.ends_with(PATH_SEPARATOR))
                })
            })
    })
}

// SC2070: `[ -n $x ]` sin comillas, que con la variable vacía da verdadero.
fn check_unquoted_non_empty(unary: Node<'_>, source: &[u8], findings: &mut Findings) {
    let in_single_bracket = unary
        .parent()
        .is_some_and(|parent| parent.kind() == TEST && first_token(parent, source) != DOUBLE_BRACKET);
    let is_non_empty = unary
        .child_by_field_name(OPERATOR_FIELD)
        .is_some_and(|operator| text(operator, source) == NON_EMPTY);
    let Some(mut operand) = named_children(unary).into_iter().last() else {
        return;
    };
    while operand.kind() == BINARY
        && let Some(left) = operand.child_by_field_name(LEFT_FIELD)
    {
        operand = left;
    }
    if in_single_bracket
        && is_non_empty
        && will_split(operand, source)
        && !parts(operand).into_iter().any(|part| is_array_part(part, source))
    {
        findings.report(ShellRule::Sc2070, operand, text(operand, source));
    }
}

// SC2091, SC2092 y SC2084: un comando que es sólo `$(cmd)`, `` `cmd` `` o `$((expr))`, que ejecuta
// lo que sale.
fn check_executed_output(command: Node<'_>, source: &[u8], findings: &mut Findings) {
    let Some(parsed) = Command::of(command) else {
        return;
    };
    let [only] = parsed.words[..] else {
        return;
    };
    let rule = match only.kind() {
        COMMAND_SUBSTITUTION if first_token(only, source) == DOLLAR_PARENTHESIS => ShellRule::Sc2091,
        COMMAND_SUBSTITUTION if first_token(only, source) == BACKTICK => ShellRule::Sc2092,
        ARITHMETIC_EXPANSION => ShellRule::Sc2084,
        _ => return,
    };
    if command.child_by_field_name(NAME_FIELD).is_some() {
        findings.report(rule, only, text(only, source));
    }
}
