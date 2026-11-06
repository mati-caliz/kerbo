use tree_sitter::Node;

use crate::rules::ShellRule;
use crate::shell::context::Findings;
use crate::shell::syntax::{
    ANSI_STRING, BODY_FIELD, COMMAND_SUBSTITUTION, DESTINATION_FIELD, FILE_REDIRECT, PIPELINE,
    REDIRECT_FIELD, REDIRECTED, children, field_children, named_children, text,
};
use crate::shell::word_pieces::{PieceKind, pieces, unquoted_literal};

const DUPLICATING_OPERATORS: &[&str] = &[">&", "<&"];
const FILE_COMMAND: &str = "file";
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

pub fn check(node: Node<'_>, source: &[u8], findings: &mut Findings) {
    match node.kind() {
        FILE_REDIRECT => check_destination(node, source, findings),
        REDIRECTED => check_dangling(node, source, findings),
        _ => {}
    }
}

// SC2210 y SC2238: `checkRedirectionToNumber` y `checkRedirectionToCommand`.
fn check_destination(redirect: Node<'_>, source: &[u8], findings: &mut Findings) {
    let Some(destination) = redirect.child_by_field_name(DESTINATION_FIELD) else {
        return;
    };
    if destination.kind() == ANSI_STRING {
        return;
    }
    let Some(operator) = children(redirect).into_iter().find(|child| !child.is_named()) else {
        return;
    };
    let word_pieces = pieces(destination, source);
    let duplicates = DUPLICATING_OPERATORS.contains(&text(operator, source));
    let is_number = unquoted_literal(&word_pieces)
        .is_some_and(|literal| literal.bytes().all(|byte| byte.is_ascii_digit()));
    if is_number && !duplicates {
        findings.report(ShellRule::Sc2210, operator, text(destination, source));
    }
    if let [only] = &word_pieces[..]
        && only.kind == PieceKind::Literal
        && only.text != FILE_COMMAND
        && COMMON_COMMANDS.contains(&only.text.as_str())
    {
        findings.report(ShellRule::Sc2238, destination, &only.text);
    }
}

// SC2188 y SC2189: `checkRedirectedNowhere`.
fn check_dangling(statement: Node<'_>, source: &[u8], findings: &mut Findings) {
    if statement.child_by_field_name(BODY_FIELD).is_some() {
        return;
    }
    let Some(first) = field_children(statement, REDIRECT_FIELD).first().copied() else {
        return;
    };
    let subject = text(first, source);
    let parent = statement.parent();
    if parent.is_some_and(|parent| parent.kind() == PIPELINE) {
        findings.report(ShellRule::Sc2189, first, subject);
        return;
    }
    let is_sole_substitution = parent
        .is_some_and(|parent| parent.kind() == COMMAND_SUBSTITUTION && named_children(parent).len() == 1);
    if !is_sole_substitution {
        findings.report(ShellRule::Sc2188, first, subject);
    }
}
