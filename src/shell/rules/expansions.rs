use tree_sitter::Node;

use crate::fix::{Fix, TextEdit, TextPoint};
use crate::rules::ShellRule;
use crate::shell::commands::Command;
use crate::shell::context::{Context, Findings};
use crate::shell::parameters::{is_array_expansion, is_counting, is_quoted_alternative, reference};
use crate::shell::quoting::{Strictness, is_quote_free, used_as_command_name};
use crate::shell::syntax::{
    ARITHMETIC_EXPANSION, ARITHMETIC_FOR, ARRAY, BINARY, BODY_FIELD, COMMAND_SUBSTITUTION, COMPOUND,
    CONCATENATION, EXPANSIONS, FILE_REDIRECT, PARENTHESIZED, POSTFIX, REDIRECTED, STRING, SUBSCRIPT, TERNARY,
    UNARY, WORD, children, is_backticked, named_children, text, token_start,
};
use crate::shell::words::{braced_content, literal_or, parts};

const SPLIT_ON_PURPOSE: &[&str] = &["seq", "pgrep"];
const ECHO: &str = "echo";
const OPENING: &str = "$(";
const CLOSING: &str = ")";
const BACKTICK_WIDTH: usize = 1;
const FLAG_PREFIX: char = '-';
const GLOB_CHARACTERS: &[char] = &['*', '?', '['];
const ESCAPE: char = '\\';
const WITHOUT_SPACES: &[&str] = &[
    "-",
    "$",
    "?",
    "!",
    "#",
    "BASHPID",
    "BASH_ARGC",
    "BASH_LINENO",
    "BASH_SUBSHELL",
    "EUID",
    "EPOCHREALTIME",
    "EPOCHSECONDS",
    "LINENO",
    "OPTIND",
    "PPID",
    "RANDOM",
    "READLINE_ARGUMENT",
    "READLINE_MARK",
    "READLINE_POINT",
    "SECONDS",
    "SHELLOPTS",
    "SHLVL",
    "SRANDOM",
    "UID",
    "COLUMNS",
    "HISTFILESIZE",
    "HISTSIZE",
    "LINES",
    "BASH_MONOSECONDS",
    "BASH_TRAPSIG",
    "FLAGS_ERROR",
    "FLAGS_FALSE",
    "FLAGS_TRUE",
];

pub fn check(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    if node.kind() == COMMAND_SUBSTITUTION {
        check_substitution(node, context, findings);
    } else if EXPANSIONS.contains(&node.kind()) {
        check_array_expansion(node, context, findings);
    } else if node.kind() == ARRAY {
        check_array_splitting(node, context, findings);
    }
}

fn check_substitution(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let statements = named_children(node);
    if statements.is_empty() {
        return;
    }
    if is_backticked(node, context.source) {
        findings.report_fixed(ShellRule::Sc2006, node, ("", backtick_fix(node, context.source)));
    }
    let single = single_command(&statements);
    let split_on_purpose = single
        .as_ref()
        .and_then(|command| command.name(context.source))
        .is_some_and(|name| SPLIT_ON_PURPOSE.contains(&name.as_str()));
    if !split_on_purpose
        && !is_quote_free(node, context, Strictness::Lenient)
        && !used_as_command_name(node, context.source)
    {
        findings.report(ShellRule::Sc2046, node, context.text(node));
    }
    if let Some(command) = single {
        check_useless_echo(node, &command, context, findings);
    }
}

fn single_command<'tree>(statements: &[Node<'tree>]) -> Option<Command<'tree>> {
    let [statement] = statements else {
        return None;
    };
    let command = if statement.kind() == REDIRECTED {
        statement.child_by_field_name(BODY_FIELD)?
    } else {
        *statement
    };
    Command::of(command)
}

fn backtick_fix(node: Node<'_>, source: &[u8]) -> Fix {
    let (_, opening) = token_start(node.start_byte(), node.start_position(), source);
    let start = TextPoint::from_point(opening);
    let end = TextPoint::from_point(node.end_position());
    let after_open = TextPoint {
        line: start.line,
        column: start.column.saturating_add(BACKTICK_WIDTH),
    };
    let before_close = TextPoint {
        line: end.line,
        column: end.column.saturating_sub(BACKTICK_WIDTH),
    };
    Fix::from_edits(
        source,
        vec![
            TextEdit {
                start,
                end: after_open,
                replacement: OPENING.to_owned(),
            },
            TextEdit {
                start: before_close,
                end,
                replacement: CLOSING.to_owned(),
            },
        ],
    )
}

fn check_useless_echo(
    node: Node<'_>,
    command: &Command<'_>,
    context: &Context<'_, '_>,
    findings: &mut Findings,
) {
    if !command.is_exactly(ECHO, context.source) {
        return;
    }
    let source = context.source;
    let Some((first, rest)) = command.arguments().split_first() else {
        return;
    };
    let covered = rest.is_empty() && is_just_command_output(*first, source);
    let flagged = literal_or(*first, source, "").starts_with(FLAG_PREFIX);
    if !covered
        && !flagged
        && command
            .arguments()
            .iter()
            .all(|word| could_be_optimized(*word, source))
    {
        findings.report(ShellRule::Sc2116, node, context.text(node));
    }
}

pub fn is_just_command_output(word: Node<'_>, source: &[u8]) -> bool {
    let substitution = match word.kind() {
        COMMAND_SUBSTITUTION => Some(word),
        STRING => match named_children(word).as_slice() {
            [only]
                if only.kind() == COMMAND_SUBSTITUTION
                    && !text(*only, source).starts_with(char::is_whitespace) =>
            {
                Some(*only)
            }
            _ => None,
        },
        _ => None,
    };
    substitution.is_some_and(|substitution| match named_children(substitution).as_slice() {
        [statement] => statement.kind() != FILE_REDIRECT && !is_only_redirection(*statement),
        _ => false,
    })
}

fn is_only_redirection(statement: Node<'_>) -> bool {
    Command::of(statement).is_some_and(|command| command.words.is_empty() && command.assignments.is_empty())
        || (statement.kind() == REDIRECTED
            && statement
                .child_by_field_name(BODY_FIELD)
                .is_none_or(is_only_redirection))
}

fn could_be_optimized(word: Node<'_>, source: &[u8]) -> bool {
    match word.kind() {
        WORD => !has_glob(text(word, source)),
        CONCATENATION => children(word)
            .into_iter()
            .all(|part| could_be_optimized(part, source)),
        _ => true,
    }
}

fn has_glob(raw: &str) -> bool {
    let mut escaped = false;
    for character in raw.chars() {
        if escaped {
            escaped = false;
        } else if character == ESCAPE {
            escaped = true;
        } else if GLOB_CHARACTERS.contains(&character) {
            return true;
        }
    }
    false
}

fn check_array_expansion(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let Some(word) = word_of_direct_part(node) else {
        return;
    };
    let source = context.source;
    let first_array = parts(word)
        .into_iter()
        .find(|part| EXPANSIONS.contains(&part.kind()) && is_array_expansion(&braced_content(*part, source)));
    if first_array != Some(node) || is_quoted_alternative(&braced_content(node, source)) {
        return;
    }
    if !is_quote_free(word, context, Strictness::Strict) {
        findings.report(ShellRule::Sc2068, node, context.text(node));
    }
}

fn word_of_direct_part(node: Node<'_>) -> Option<Node<'_>> {
    let parent = node.parent()?;
    match parent.kind() {
        CONCATENATION => Some(parent),
        STRING => None,
        kind if EXPANSIONS.contains(&kind) || is_arithmetic_parent(kind) => None,
        _ => Some(node),
    }
}

fn is_arithmetic_parent(kind: &str) -> bool {
    [
        BINARY,
        UNARY,
        POSTFIX,
        PARENTHESIZED,
        TERNARY,
        ARITHMETIC_EXPANSION,
        COMPOUND,
        ARITHMETIC_FOR,
        SUBSCRIPT,
    ]
    .contains(&kind)
}

fn check_array_splitting(array: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let source = context.source;
    for element in named_children(array) {
        if is_indexed_element(element, source) {
            continue;
        }
        let element_parts = if element.kind() == CONCATENATION {
            children(element)
        } else {
            vec![element]
        };
        for part in element_parts {
            check_array_part(part, context, findings);
        }
    }
}

fn check_array_part(part: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    if part.kind() == COMMAND_SUBSTITUTION {
        findings.report(ShellRule::Sc2207, part, context.text(part));
        return;
    }
    if !EXPANSIONS.contains(&part.kind()) {
        return;
    }
    let braced = braced_content(part, context.source);
    if !is_counting(&braced)
        && !is_quoted_alternative(&braced)
        && !WITHOUT_SPACES.contains(&reference(&braced).as_str())
    {
        findings.report(ShellRule::Sc2206, part, context.text(part));
    }
}

fn is_indexed_element(element: Node<'_>, source: &[u8]) -> bool {
    let raw = text(element, source);
    raw.starts_with('[') && raw.contains("]=")
}
