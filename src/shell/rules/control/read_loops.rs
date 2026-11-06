use tree_sitter::Node;

use crate::rules::ShellRule;
use crate::shell::commands::Command;
use crate::shell::context::{Findings, Position};
use crate::shell::syntax::{
    ARITHMETIC_FOR, ASSIGNMENT, ASSIGNMENTS, BODY_FIELD, COMMAND, COMMAND_SUBSTITUTION, COMPOUND,
    CONCATENATION, CONDITION_FIELD, DESCRIPTOR_FIELD, ELIF_CLAUSE, ELSE_CLAUSE, FILE_REDIRECT, FOR,
    HEREDOC_REDIRECT, HERESTRING, IF, PIPELINE, REDIRECT_FIELD, REDIRECTED, STRING, SUBSHELL, VALUE_FIELD,
    WHILE, children, field_children, is_backticked, named_children, text,
};
use crate::shell::word_pieces::{literal_string, pieces};
use crate::shell::words::oversimplify;

const WHILE_KEYWORD: &str = "while";
const READ: &str = "read";
const OTHER_DESCRIPTOR_FLAG: &str = "-u";
const STDIN: &str = "0";
const READ_REDIRECT: &str = "<";
const DUPLICATE_STDIN: &str = "<&";
const PATH_SEPARATOR: char = '/';

enum Exemption {
    Flag(&'static str),
    Argument(&'static str),
    Never,
}

// Los comandos que se comen la entrada del loop, con lo que los deja afuera.
const SWALLOWERS: &[(&str, Exemption)] = &[
    ("ssh", Exemption::Flag("n")),
    ("ffmpeg", Exemption::Argument("-nostdin")),
    ("mplayer", Exemption::Argument("-noconsolecontrols")),
    ("HandBrakeCLI", Exemption::Never),
];

// SC2095: `checkWhileReadPitfalls`.
pub fn check(node: Node<'_>, source: &[u8], findings: &mut Findings) {
    if node.kind() != WHILE || !is_while_keyword(node, source) {
        return;
    }
    let conditions: Vec<Node<'_>> = field_children(node, CONDITION_FIELD)
        .into_iter()
        .filter(Node::is_named)
        .collect();
    let [condition] = conditions[..] else {
        return;
    };
    if !is_stdin_read(condition, source) {
        return;
    }
    let Some(body) = node.child_by_field_name(BODY_FIELD) else {
        return;
    };
    for statement in named_children(body) {
        check_statement(statement, Position::of(node), source, findings);
    }
}

fn is_while_keyword(node: Node<'_>, source: &[u8]) -> bool {
    children(node)
        .first()
        .is_some_and(|keyword| text(*keyword, source) == WHILE_KEYWORD)
}

fn is_stdin_read(statement: Node<'_>, source: &[u8]) -> bool {
    let (body, redirects) = split_redirects(statement);
    let Some(command) = Command::of(body) else {
        return false;
    };
    let words: Vec<String> = command
        .words
        .iter()
        .map(|word| oversimplify(*word, source))
        .collect();
    words.first().is_some_and(|first| first == READ)
        && !words.iter().any(|word| word == OTHER_DESCRIPTOR_FLAG)
        && !redirects
            .iter()
            .any(|redirect| redirects_stdin(*redirect, source))
}

fn split_redirects(statement: Node<'_>) -> (Node<'_>, Vec<Node<'_>>) {
    if statement.kind() == REDIRECTED {
        let body = statement.child_by_field_name(BODY_FIELD).unwrap_or(statement);
        let mut redirects = field_children(statement, REDIRECT_FIELD);
        redirects.extend(field_children(body, REDIRECT_FIELD));
        return (body, redirects);
    }
    (statement, field_children(statement, REDIRECT_FIELD))
}

fn redirects_stdin(redirect: Node<'_>, source: &[u8]) -> bool {
    if let Some(descriptor) = redirect.child_by_field_name(DESCRIPTOR_FIELD) {
        return text(descriptor, source) == STDIN;
    }
    match redirect.kind() {
        FILE_REDIRECT => children(redirect).into_iter().any(|child| {
            !child.is_named() && [READ_REDIRECT, DUPLICATE_STDIN].contains(&text(child, source))
        }),
        HERESTRING | HEREDOC_REDIRECT => true,
        _ => false,
    }
}

fn check_statement(statement: Node<'_>, loop_start: Position, source: &[u8], findings: &mut Findings) {
    let first_stage = if statement.kind() == PIPELINE {
        match named_children(statement).first() {
            Some(first) => *first,
            None => return,
        }
    } else {
        statement
    };
    if [ASSIGNMENT, ASSIGNMENTS].contains(&first_stage.kind()) {
        let assignments = if first_stage.kind() == ASSIGNMENT {
            vec![first_stage]
        } else {
            named_children(first_stage)
        };
        for assignment in assignments {
            check_word_substitutions(assignment, loop_start, source, findings);
        }
        return;
    }
    if ![
        COMMAND,
        REDIRECTED,
        IF,
        WHILE,
        FOR,
        ARITHMETIC_FOR,
        COMPOUND,
        SUBSHELL,
    ]
    .contains(&first_stage.kind())
    {
        return;
    }
    let (body, redirects) = split_redirects(first_stage);
    if let Some(command) = Command::of(body) {
        for word in command.words.iter().chain(command.assignments.iter()) {
            check_word_substitutions(*word, loop_start, source, findings);
        }
    }
    if redirects
        .iter()
        .any(|redirect| redirects_stdin(*redirect, source))
    {
        return;
    }
    for nested in command_sequences(body, source) {
        check_statement(nested, loop_start, source, findings);
    }
    if let Some(command) = Command::of(body) {
        check_swallower(&command, loop_start, source, findings);
    }
}

fn check_word_substitutions(word: Node<'_>, loop_start: Position, source: &[u8], findings: &mut Findings) {
    let value = word.child_by_field_name(VALUE_FIELD).unwrap_or(word);
    for part in substitutions(value) {
        for statement in named_children(part) {
            check_statement(statement, loop_start, source, findings);
        }
    }
}

fn substitutions(word: Node<'_>) -> Vec<Node<'_>> {
    match word.kind() {
        COMMAND_SUBSTITUTION => vec![word],
        CONCATENATION | STRING => named_children(word).into_iter().flat_map(substitutions).collect(),
        _ => Vec::new(),
    }
}

// `getCommandSequences`.
fn command_sequences<'tree>(body: Node<'tree>, source: &[u8]) -> Vec<Node<'tree>> {
    match body.kind() {
        COMPOUND | SUBSHELL if !is_backticked(body, source) => named_children(body),
        WHILE => {
            let mut statements: Vec<Node<'tree>> = field_children(body, CONDITION_FIELD)
                .into_iter()
                .filter(Node::is_named)
                .collect();
            statements.extend(
                body.child_by_field_name(BODY_FIELD)
                    .map_or_else(Vec::new, named_children),
            );
            statements
        }
        FOR | ARITHMETIC_FOR => body
            .child_by_field_name(BODY_FIELD)
            .map_or_else(Vec::new, named_children),
        IF => {
            let mut statements = Vec::new();
            for child in named_children(body) {
                if [ELIF_CLAUSE, ELSE_CLAUSE].contains(&child.kind()) {
                    statements.extend(named_children(child));
                } else {
                    statements.push(child);
                }
            }
            statements
        }
        _ => Vec::new(),
    }
}

fn check_swallower(command: &Command<'_>, loop_start: Position, source: &[u8], findings: &mut Findings) {
    let Some(name) = command.name(source) else {
        return;
    };
    let basename = name.rsplit(PATH_SEPARATOR).next().unwrap_or(&name);
    let Some((_, exemption)) = SWALLOWERS.iter().find(|(swallower, _)| *swallower == basename) else {
        return;
    };
    let exempted = match exemption {
        Exemption::Flag(flag) => command.has_flag(flag, source),
        Exemption::Argument(argument) => command
            .arguments()
            .iter()
            .any(|word| literal_string(&pieces(*word, source)).as_deref() == Some(*argument)),
        Exemption::Never => false,
    };
    if !exempted {
        findings.report_at(ShellRule::Sc2095, loop_start, basename);
        findings.report(ShellRule::Sc2095, command.node, basename);
    }
}
