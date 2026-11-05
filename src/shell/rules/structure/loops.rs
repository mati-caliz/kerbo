use tree_sitter::Node;

use crate::rules::ShellRule;
use crate::shell::commands::Command;
use crate::shell::context::{Context, Findings, Position};
use crate::shell::dialect::Dialect;
use crate::shell::parameters::modifier;
use crate::shell::pattern_scan::{SegmentKind, segments};
use crate::shell::syntax::{
    BODY_FIELD, COMMAND, COMMAND_SUBSTITUTION, EXPANSION, EXPANSIONS, FOR, OPERATOR_FIELD, PIPELINE,
    RAW_STRING, REDIRECT_FIELD, REDIRECTED, STRING, VALUE_FIELD, children, field_children, first_token,
    named_children, text,
};
use crate::shell::word_pieces::{PieceKind, pieces};
use crate::shell::words::{braced_content, oversimplify};

const SELECT: &str = "select";
const LS: &str = "ls";
const FIND: &str = "find";
const ARGUMENT_ENDS: &[&str] = &["--", ":::", "::::"];
const GLOB_OPTION_SAFE_COMMANDS: &[&str] = &["echo", "printf"];
const OPTION_LIKE_GLOBS: &[&str] = &["*", "?"];
const PATH_SEPARATOR: char = '/';
const SED: &str = "sed";
const ECHO: &str = "echo";
const EXPRESSION_FLAG: &str = "-e";
const EXPANDED_PLACEHOLDER: &str = "${VAR}";
const SUBSTITUTE: char = 's';
const LINE_FEED: char = '\n';
const EXPECTED_DELIMITERS: usize = 2;
const HERESTRING: &str = "herestring_redirect";
const PATTERN_OPERATORS: &[char] = &['%', '#'];

pub fn check(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let source = context.source;
    match node.kind() {
        FOR if first_token(node, source) != SELECT => {
            check_listing_loop(node, source, findings);
            check_glob_variables(node, source, findings);
        }
        COMMAND => {
            check_globs_as_options(node, source, findings);
            check_sed_herestring(node, context, findings);
        }
        PIPELINE => check_echo_sed(node, context, findings),
        EXPANSION => check_pattern_expansions(node, source, findings),
        _ => {}
    }
}

// SC2044 y SC2045: `checkForInLs`.
fn check_listing_loop(loop_node: Node<'_>, source: &[u8], findings: &mut Findings) {
    let [value] = field_children(loop_node, VALUE_FIELD)[..] else {
        return;
    };
    if value.kind() != COMMAND_SUBSTITUTION {
        return;
    }
    let [statement] = named_children(value)[..] else {
        return;
    };
    let simple = if statement.kind() == REDIRECTED {
        statement.child_by_field_name(BODY_FIELD).unwrap_or(statement)
    } else {
        statement
    };
    let Some(command) = Command::of(simple) else {
        return;
    };
    match oversimplify_first(&command, source).as_str() {
        LS => findings.report(ShellRule::Sc2045, value, LS),
        FIND => findings.report(ShellRule::Sc2044, value, FIND),
        _ => {}
    }
}

fn oversimplify_first(command: &Command<'_>, source: &[u8]) -> String {
    command
        .first_word()
        .map(|word| oversimplify(word, source))
        .unwrap_or_default()
}

// SC2231: `checkForLoopGlobVariables`.
fn check_glob_variables(loop_node: Node<'_>, source: &[u8], findings: &mut Findings) {
    for word in field_children(loop_node, VALUE_FIELD) {
        let word_pieces = pieces(word, source);
        let globbed = word_pieces
            .iter()
            .any(|piece| matches!(piece.kind, PieceKind::Glob | PieceKind::Extglob));
        if !globbed {
            continue;
        }
        for piece in word_pieces
            .iter()
            .filter(|piece| matches!(piece.kind, PieceKind::Braced | PieceKind::CommandSubstitution))
        {
            findings.report_at(ShellRule::Sc2231, piece.position, text(word, source));
        }
    }
}

// SC2035: `checkGlobsAsOptions`.
fn check_globs_as_options(node: Node<'_>, source: &[u8], findings: &mut Findings) {
    let Some(command) = Command::of(node) else {
        return;
    };
    let basename = command
        .name(source)
        .map(|name| name.rsplit(PATH_SEPARATOR).next().unwrap_or_default().to_owned());
    if basename.is_some_and(|name| GLOB_OPTION_SAFE_COMMANDS.contains(&name.as_str())) {
        return;
    }
    for argument in command
        .arguments()
        .iter()
        .take_while(|argument| !ARGUMENT_ENDS.contains(&oversimplify(**argument, source).as_str()))
    {
        let word_pieces = pieces(*argument, source);
        if word_pieces.first().is_some_and(|first| {
            first.kind == PieceKind::Glob && OPTION_LIKE_GLOBS.contains(&first.text.as_str())
        }) {
            findings.report(ShellRule::Sc2035, *argument, text(*argument, source));
        }
    }
}

// SC2001: `checkEchoSed`, sólo en bash y ksh.
fn check_echo_sed(pipeline: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    if !has_replacement_expansion(context.dialect) {
        return;
    }
    let source = context.source;
    let [first, second] = named_children(pipeline)[..] else {
        return;
    };
    let (Some(echo), Some(sed)) = (Command::of(first), Command::of(second)) else {
        return;
    };
    let echoed: Vec<String> = echo
        .words
        .iter()
        .map(|word| oversimplify(*word, source))
        .collect();
    if echoed == [ECHO, EXPANDED_PLACEHOLDER] && is_simple_sed(&sed, source) {
        findings.report(ShellRule::Sc2001, pipeline, SED);
    }
}

fn check_sed_herestring(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let has_herestring = field_children(node, REDIRECT_FIELD)
        .into_iter()
        .any(|redirect| redirect.kind() == HERESTRING);
    if !has_herestring || !has_replacement_expansion(context.dialect) {
        return;
    }
    if Command::of(node).is_some_and(|command| is_simple_sed(&command, context.source)) {
        findings.report(ShellRule::Sc2001, node, SED);
    }
}

const fn has_replacement_expansion(dialect: Dialect) -> bool {
    matches!(dialect, Dialect::Bash | Dialect::Ksh)
}

fn is_simple_sed(command: &Command<'_>, source: &[u8]) -> bool {
    let words: Vec<String> = command
        .words
        .iter()
        .map(|word| oversimplify(*word, source))
        .collect();
    let ([SED, expression] | [SED, EXPRESSION_FLAG, expression]) =
        words.iter().map(String::as_str).collect::<Vec<_>>()[..]
    else {
        return false;
    };
    is_simple_substitution(expression)
}

// `^s(.)([^\n]*)g?$` con el delimitador exactamente dos veces en lo que sigue.
fn is_simple_substitution(expression: &str) -> bool {
    let mut characters = expression.chars();
    if characters.next() != Some(SUBSTITUTE) {
        return false;
    }
    let Some(delimiter) = characters.next() else {
        return false;
    };
    let rest = characters.as_str();
    !rest.contains(LINE_FEED) && rest.matches(delimiter).count() == EXPECTED_DELIMITERS
}

// SC2295: `checkUnquotedParameterExpansionPattern`.
fn check_pattern_expansions(expansion: Node<'_>, source: &[u8], findings: &mut Findings) {
    if !modifier(&braced_content(expansion, source)).starts_with(PATTERN_OPERATORS) {
        return;
    }
    let mut past_operator = false;
    for child in children(expansion) {
        if !past_operator {
            let is_operator = expansion
                .child_by_field_name(OPERATOR_FIELD)
                .is_some_and(|operator| operator.id() == child.id());
            past_operator = is_operator;
            continue;
        }
        if EXPANSIONS.contains(&child.kind()) || child.kind() == COMMAND_SUBSTITUTION {
            findings.report(ShellRule::Sc2295, child, text(child, source));
        } else if child.is_named() && ![STRING, RAW_STRING].contains(&child.kind()) {
            let unquoted = segments(text(child, source))
                .into_iter()
                .filter(|segment| matches!(segment.kind, SegmentKind::Expansion | SegmentKind::Substitution));
            for segment in unquoted {
                let position = Position::of(child).shifted(segment.start);
                findings.report_at(ShellRule::Sc2295, position, text(child, source));
            }
        }
    }
}
