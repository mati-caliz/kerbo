use tree_sitter::Node;

use crate::rules::ShellRule;
use crate::shell::commands::Command;
use crate::shell::context::{Context, Findings};
use crate::shell::syntax::{
    ARITHMETIC_EXPANSION, ASSIGNMENT, ASSIGNMENTS, COMMAND, INDEX_FIELD, NAME_FIELD, SUBSCRIPT, VALUE_FIELD,
    children, is_statement, named_children, text,
};
use crate::shell::word_pieces::{Piece, PieceKind, flattened, has_glob, pieces, unquoted_literal};
use crate::shell::words::literal;

const IFS: &str = "IFS";
const IFS_SUSPICIOUS: &[char] = &['\\', 'n', 't'];
const PLAIN_ASSIGNMENT: &str = "=";
const FLAG_PREFIX: char = '-';
const DECIMAL_POINT: char = '.';
const COMMON_COMMANDS: &[&str] = &[
    "admin",
    "alias",
    "ar",
    "asa",
    "at",
    "awk",
    "basename",
    "batch",
    "bc",
    "bg",
    "break",
    "c99",
    "cal",
    "cat",
    "cd",
    "cflow",
    "chgrp",
    "chmod",
    "chown",
    "cksum",
    "cmp",
    "colon",
    "comm",
    "command",
    "compress",
    "continue",
    "cp",
    "crontab",
    "csplit",
    "ctags",
    "cut",
    "cxref",
    "date",
    "dd",
    "delta",
    "df",
    "diff",
    "dirname",
    "dot",
    "du",
    "echo",
    "ed",
    "env",
    "eval",
    "ex",
    "exec",
    "exit",
    "expand",
    "export",
    "expr",
    "fc",
    "fg",
    "file",
    "find",
    "fold",
    "fort77",
    "fuser",
    "gencat",
    "get",
    "getconf",
    "getopts",
    "grep",
    "hash",
    "head",
    "iconv",
    "ipcrm",
    "ipcs",
    "jobs",
    "join",
    "kill",
    "lex",
    "link",
    "ln",
    "locale",
    "localedef",
    "logger",
    "logname",
    "lp",
    "ls",
    "m4",
    "mailx",
    "make",
    "man",
    "mesg",
    "mkdir",
    "mkfifo",
    "more",
    "mv",
    "newgrp",
    "nice",
    "nl",
    "nm",
    "nohup",
    "od",
    "paste",
    "patch",
    "pathchk",
    "pax",
    "pr",
    "printf",
    "prs",
    "ps",
    "pwd",
    "qalter",
    "qdel",
    "qhold",
    "qmove",
    "qmsg",
    "qrerun",
    "qrls",
    "qselect",
    "qsig",
    "qstat",
    "qsub",
    "read",
    "readonly",
    "renice",
    "return",
    "rm",
    "rmdel",
    "rmdir",
    "sact",
    "sccs",
    "sed",
    "set",
    "sh",
    "shift",
    "sleep",
    "sort",
    "split",
    "strings",
    "strip",
    "stty",
    "tabs",
    "tail",
    "talk",
    "tee",
    "test",
    "time",
    "times",
    "touch",
    "tput",
    "tr",
    "trap",
    "tsort",
    "tty",
    "type",
    "ulimit",
    "umask",
    "unalias",
    "uname",
    "uncompress",
    "unexpand",
    "unget",
    "uniq",
    "unlink",
    "unset",
    "uucp",
    "uudecode",
    "uuencode",
    "uustat",
    "uux",
    "val",
    "vi",
    "wait",
    "wc",
    "what",
    "who",
    "write",
    "xargs",
    "yacc",
    "zcat",
];

pub fn check(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let source = context.source;
    if node.kind() == COMMAND {
        check_assignment_ate_command(node, source, findings);
    }
    if node.kind() == ASSIGNMENTS {
        check_flag_read_as_assignment(node, source, findings);
    }
    if node.kind() != ASSIGNMENT {
        return;
    }
    check_ifs(node, source, findings);
    check_index(node, context, findings);
    if is_statement(node) {
        check_self_assignment(node, source, findings);
        check_command_name_value(node, source, findings);
    }
}

// SC2037 y SC2209: `checkAssignAteCommand`, con palabras después de la asignación.
fn check_assignment_ate_command(command_node: Node<'_>, source: &[u8], findings: &mut Findings) {
    let Some(command) = Command::of(command_node) else {
        return;
    };
    let ([assignment], Some(first_word)) = (command.assignments.as_slice(), command.first_word()) else {
        return;
    };
    let first_pieces = pieces(first_word, source);
    if has_glob(&first_pieces) || is_unquoted_flag(&first_pieces) {
        findings.report(ShellRule::Sc2037, command_node, text(*assignment, source));
    } else {
        check_command_name_value(*assignment, source, findings);
    }
}

// tree-sitter lee `A=ls --sort=$x` como dos asignaciones.
fn check_flag_read_as_assignment(assignments: Node<'_>, source: &[u8], findings: &mut Findings) {
    let [_, second, ..] = named_children(assignments)[..] else {
        return;
    };
    let is_flag = second
        .child_by_field_name(NAME_FIELD)
        .is_some_and(|name| text(name, source).starts_with(FLAG_PREFIX));
    if is_flag {
        findings.report(ShellRule::Sc2037, assignments, text(second, source));
    }
}

fn is_unquoted_flag(word_pieces: &[Piece]) -> bool {
    word_pieces
        .first()
        .is_some_and(|first| first.kind == PieceKind::Literal && first.text.starts_with(FLAG_PREFIX))
}

fn check_command_name_value(assignment: Node<'_>, source: &[u8], findings: &mut Findings) {
    let Some(value) = assignment.child_by_field_name(VALUE_FIELD) else {
        return;
    };
    let is_common_command = unquoted_literal(&pieces(value, source))
        .is_some_and(|literal_value| COMMON_COMMANDS.contains(&literal_value.as_str()));
    if is_common_command {
        findings.report(ShellRule::Sc2209, assignment, text(value, source));
    }
}

// SC2141: `checkSuspiciousIFS`.
fn check_ifs(assignment: Node<'_>, source: &[u8], findings: &mut Findings) {
    let is_ifs = assignment
        .child_by_field_name(NAME_FIELD)
        .is_some_and(|name| text(name, source) == IFS);
    let Some(value) = assignment.child_by_field_name(VALUE_FIELD).filter(|_| is_ifs) else {
        return;
    };
    if literal(value, source).is_some_and(|literal_value| literal_value.contains(IFS_SUSPICIOUS)) {
        findings.report(ShellRule::Sc2141, value, text(value, source));
    }
}

// SC2269: `checkAssignToSelf`.
fn check_self_assignment(assignment: Node<'_>, source: &[u8], findings: &mut Findings) {
    let is_plain = children(assignment)
        .into_iter()
        .any(|child| !child.is_named() && text(child, source) == PLAIN_ASSIGNMENT);
    let (Some(name), Some(value)) = (
        assignment.child_by_field_name(NAME_FIELD),
        assignment.child_by_field_name(VALUE_FIELD),
    ) else {
        return;
    };
    let value_pieces = pieces(value, source);
    if let ([only], true) = (flattened(&value_pieces).as_slice(), is_plain)
        && only.kind == PieceKind::Braced
        && only.text == text(name, source)
    {
        findings.report(ShellRule::Sc2269, assignment, text(name, source));
    }
}

// SC2321 y SC2079: el índice de `a[i]=x`, que los arreglos asociativos no evalúan.
fn check_index(assignment: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let source = context.source;
    let Some(subscript) = assignment
        .child_by_field_name(NAME_FIELD)
        .filter(|name| name.kind() == SUBSCRIPT)
    else {
        return;
    };
    let Some(name) = subscript.child_by_field_name(NAME_FIELD) else {
        return;
    };
    if context.associative.contains(text(name, source)) {
        return;
    }
    let Some(index) = subscript.child_by_field_name(INDEX_FIELD) else {
        return;
    };
    if index.kind() == ARITHMETIC_EXPANSION {
        findings.report(ShellRule::Sc2321, index, text(index, source));
        return;
    }
    if named_children(index).is_empty() && is_decimal(text(index, source)) {
        findings.report(ShellRule::Sc2079, index, text(index, source));
    }
}

fn is_decimal(value: &str) -> bool {
    value.chars().next().is_some_and(|first| first.is_ascii_digit()) && value.contains(DECIMAL_POINT)
}
