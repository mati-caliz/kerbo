use tree_sitter::Node;

use crate::rules::ShellRule;
use crate::shell::commands::Command;
use crate::shell::context::{Context, Finding, Findings};
use crate::shell::dialect::Dialect;
use crate::shell::syntax::{
    ASSIGNMENT, ASSIGNMENTS, BODY_FIELD, CASE, CASE_ITEM, COMMAND, COMMENT, DECLARATION, FUNCTION,
    HEREDOC_BODY, HEREDOC_REDIRECT, NAME_FIELD, PIPELINE, REDIRECT_FIELD, REDIRECTED, VALUE_FIELD, children,
    field_children, named_children, text, token_text,
};
use crate::shell::variables::assigned_name;
use crate::shell::word_pieces::{Piece, PieceKind, literal_string, only_literal_text, pieces};
use crate::shell::words::oversimplify;

const FALLTHROUGH_FIELD: &str = "fallthrough";
const CONTINUE_CASE: &str = ";;&";
const FALLTHROUGH_CASE: &str = ";&";
const EXPANDED_PLACEHOLDER: &str = "${VAR}";
const ECHO: &str = "echo";
const WC: &str = "wc";
const COUNT_FLAGS: &[&str] = &["-c", "-m"];
const PIPE_STDERR: &str = "|&";
const LONELY_DOT_SLASH: &str = "./";
const SSH: &str = "ssh";
const FUNCTION_KEYWORD: &str = "function";
const OPEN_PARENTHESIS: &str = "(";
const PATH_VARIABLE: &str = "PATH";
const APPEND: &str = "+=";
const BIN_DIRECTORIES: &[&str] = &["/bin", "/sbin"];
const PATH_SEPARATOR: char = '/';
const PATH_LIST_SEPARATOR: char = ':';
const TILDE: char = '~';
const HEREDOC_START: &str = "heredoc_start";
const QUOTES_IN_DELIMITER: &[char] = &['\'', '"', '\\'];
const HEREDOC_CONTENT: &str = "heredoc_content";
const SHEBANG: &str = "#!";
const ASH: &str = "ash";
const SPLIT_STRING_FLAGS: &[&str] = &["-S", "--split-string"];
const ENV: &str = "env";
const FILE_START: usize = 0;
const MAX_SHEBANG_WORDS: usize = 2;

pub fn check(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let source = context.source;
    match node.kind() {
        PIPELINE => {
            check_echo_wc(node, source, findings);
            check_piped_assignment(node, source, findings);
            check_stderr_pipe(node, context, findings);
        }
        COMMAND => {
            check_lonely_dot_slash(node, source, findings);
            check_prefix_tilde(node, source, findings);
        }
        REDIRECTED => check_ssh_heredoc(node, source, findings),
        FUNCTION => check_function_declaration(node, context, findings),
        ASSIGNMENT => check_path_assignment(node, source, findings),
        CASE => check_case_terminators(node, context, findings),
        _ => {}
    }
}

fn command_words(stage: Node<'_>, source: &[u8]) -> Vec<String> {
    let body = if stage.kind() == REDIRECTED {
        stage.child_by_field_name(BODY_FIELD).unwrap_or(stage)
    } else {
        stage
    };
    Command::of(body).map_or_else(Vec::new, |command| {
        command
            .words
            .iter()
            .map(|word| oversimplify(*word, source))
            .collect()
    })
}

// SC2000: `checkEchoWc`.
fn check_echo_wc(pipeline: Node<'_>, source: &[u8], findings: &mut Findings) {
    let [first, second] = named_children(pipeline)[..] else {
        return;
    };
    let first_words = command_words(first, source);
    let second_words = command_words(second, source);
    let is_echo =
        matches!(first_words.as_slice(), [name, value] if name == ECHO && value == EXPANDED_PLACEHOLDER);
    let is_count =
        matches!(second_words.as_slice(), [name, flag] if name == WC && COUNT_FLAGS.contains(&flag.as_str()));
    if is_echo && is_count {
        findings.report(ShellRule::Sc2000, pipeline, text(pipeline, source));
    }
}

// SC2036: `checkPipedAssignment`.
fn check_piped_assignment(pipeline: Node<'_>, source: &[u8], findings: &mut Findings) {
    let Some(first) = named_children(pipeline).first().copied() else {
        return;
    };
    let body = if first.kind() == REDIRECTED {
        first.child_by_field_name(BODY_FIELD).unwrap_or(first)
    } else {
        first
    };
    let only_assignments = [ASSIGNMENT, ASSIGNMENTS].contains(&body.kind())
        || Command::of(body)
            .is_some_and(|command| command.node.kind() == COMMAND && command.words.is_empty());
    if only_assignments {
        findings.report(ShellRule::Sc2036, body, text(body, source));
    }
}

// SC2118: `checkStderrPipe`.
fn check_stderr_pipe(pipeline: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    if context.dialect != Dialect::Ksh {
        return;
    }
    for child in children(pipeline) {
        if !child.is_named() && text(child, context.source) == PIPE_STDERR {
            findings.report(ShellRule::Sc2118, child, PIPE_STDERR);
        }
    }
}

// SC2083: `checkLonelyDotDash`.
fn check_lonely_dot_slash(node: Node<'_>, source: &[u8], findings: &mut Findings) {
    let is_lonely = Command::of(node)
        .and_then(|command| command.name(source))
        .is_some_and(|name| name == LONELY_DOT_SLASH);
    if is_lonely {
        findings.report(ShellRule::Sc2083, node, LONELY_DOT_SLASH);
    }
}

// SC2147: `checkTildeInPath`, en las asignaciones que van delante de un comando.
fn check_prefix_tilde(node: Node<'_>, source: &[u8], findings: &mut Findings) {
    for assignment in Command::of(node).map_or_else(Vec::new, |command| command.assignments) {
        report_tilde_in_path(assignment, source, findings);
    }
}

fn report_tilde_in_path(assignment: Node<'_>, source: &[u8], findings: &mut Findings) {
    let Some(value) = plain_path_value(assignment, source) else {
        return;
    };
    let has_quoted_tilde = pieces(value, source).iter().any(|piece| {
        matches!(piece.kind, PieceKind::DoubleQuoted | PieceKind::SingleQuoted)
            && only_literal_text(std::slice::from_ref(piece), "").contains(TILDE)
    });
    if has_quoted_tilde {
        findings.report(ShellRule::Sc2147, assignment, PATH_VARIABLE);
    }
}

fn plain_path_value<'tree>(assignment: Node<'tree>, source: &[u8]) -> Option<Node<'tree>> {
    let is_path = assignment
        .child_by_field_name(NAME_FIELD)
        .is_some_and(|name| text(name, source) == PATH_VARIABLE);
    let appends = children(assignment)
        .into_iter()
        .any(|child| !child.is_named() && text(child, source) == APPEND);
    if is_path && !appends {
        assignment.child_by_field_name(VALUE_FIELD)
    } else {
        None
    }
}

// SC2123 y la parte de SC2147 de las asignaciones sueltas: `checkOverridingPath` y `checkTildeInPath`.
fn check_path_assignment(assignment: Node<'_>, source: &[u8], findings: &mut Findings) {
    let is_alone = assignment
        .parent()
        .is_none_or(|parent| ![COMMAND, DECLARATION].contains(&parent.kind()));
    if !is_alone || assigned_name(assignment, source).as_deref() != Some(PATH_VARIABLE) {
        return;
    }
    report_tilde_in_path(assignment, source, findings);
    let is_plain = assignment
        .child_by_field_name(NAME_FIELD)
        .is_some_and(|name| text(name, source) == PATH_VARIABLE)
        && !children(assignment)
            .into_iter()
            .any(|child| !child.is_named() && text(child, source) == APPEND);
    if !is_plain {
        return;
    }
    let value = assignment.child_by_field_name(VALUE_FIELD);
    let word_pieces: Vec<Piece> = value.map_or_else(Vec::new, |value| pieces(value, source));
    let string = value.map_or_else(String::new, |value| oversimplify(value, source));
    if BIN_DIRECTORIES.iter().any(|directory| string.contains(directory)) {
        return;
    }
    let has_separator = string.contains(PATH_LIST_SEPARATOR);
    let has_slash = string.contains(PATH_SEPARATOR);
    let is_literal = literal_string(&word_pieces).is_some();
    if !has_separator && (has_slash || is_literal) {
        findings.report(ShellRule::Sc2123, assignment, PATH_VARIABLE);
    }
}

// SC2087: `checkSshHereDoc`.
fn check_ssh_heredoc(statement: Node<'_>, source: &[u8], findings: &mut Findings) {
    let is_ssh = statement
        .child_by_field_name(BODY_FIELD)
        .and_then(Command::of)
        .is_some_and(|command| command.is_command(SSH, source));
    if !is_ssh {
        return;
    }
    for redirect in field_children(statement, REDIRECT_FIELD) {
        if redirect.kind() != HEREDOC_REDIRECT {
            continue;
        }
        let Some(start) = named_children(redirect)
            .into_iter()
            .find(|child| child.kind() == HEREDOC_START)
        else {
            continue;
        };
        let unquoted = !text(start, source).contains(QUOTES_IN_DELIMITER);
        let expands = named_children(redirect)
            .iter()
            .filter(|child| child.kind() == HEREDOC_BODY)
            .flat_map(|body| named_children(*body))
            .any(|part| part.kind() != HEREDOC_CONTENT);
        if unquoted && expands {
            findings.report(ShellRule::Sc2087, start, SSH);
        }
    }
}

// SC2111, SC2112 y SC2113: `checkFunctionDeclarations`.
fn check_function_declaration(function: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let source = context.source;
    let tokens = children(function);
    let has_keyword = tokens
        .first()
        .is_some_and(|first| text(*first, source) == FUNCTION_KEYWORD);
    let has_parentheses = tokens
        .iter()
        .any(|token| !token.is_named() && text(*token, source) == OPEN_PARENTHESIS);
    let rule = match context.dialect {
        Dialect::Bash => None,
        Dialect::Ksh => (has_keyword && has_parentheses).then_some(ShellRule::Sc2111),
        Dialect::Sh | Dialect::Dash | Dialect::Busybox => match (has_keyword, has_parentheses) {
            (true, true) => Some(ShellRule::Sc2112),
            (true, false) => Some(ShellRule::Sc2113),
            _ => None,
        },
    };
    if let Some(rule) = rule {
        findings.report(rule, function, FUNCTION_KEYWORD);
    }
}

// SC2127: `checkUnsupported`, los `;&` y `;;&` de un `case`.
fn check_case_terminators(case: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let source = context.source;
    let terminators: Vec<&str> = named_children(case)
        .into_iter()
        .filter(|child| child.kind() == CASE_ITEM)
        .filter_map(|item| item.child_by_field_name(FALLTHROUGH_FIELD))
        .map(|terminator| token_text(terminator, source))
        .collect();
    let unsupported = if terminators.contains(&CONTINUE_CASE) {
        context.dialect != Dialect::Bash
    } else if terminators.contains(&FALLTHROUGH_CASE) {
        !matches!(context.dialect, Dialect::Bash | Dialect::Ksh)
    } else {
        false
    };
    if unsupported {
        findings.report(ShellRule::Sc2127, case, "case");
    }
}

// `executableFromShebang`: lo que corre `env`, si es que lo corre `env`.
fn shebang_executable<'shebang>(words: &[&'shebang str]) -> &'shebang str {
    let base = |word: &'shebang str| word.rsplit(PATH_SEPARATOR).next().unwrap_or(word);
    let Some(first) = words.first().map(|word| base(word)) else {
        return "";
    };
    if first != ENV {
        return first;
    }
    words
        .iter()
        .skip(1)
        .find(|word| !word.starts_with('-') && !word.contains('='))
        .map_or("", |word| base(word))
}

// SC2096, SC2187, SC2239 y SC2246: `checkShebangParameters` y `checkShebang`.
pub fn check_shebang(context: &Context<'_, '_>, findings: &mut Findings) {
    let source = context.source;
    let Some(first) = context
        .root
        .child(0)
        .filter(|first| first.kind() == COMMENT && first.start_byte() == FILE_START)
    else {
        return;
    };
    let Some(shebang) = text(first, source).strip_prefix(SHEBANG).map(str::trim_start) else {
        return;
    };
    let words: Vec<&str> = shebang.split_whitespace().collect();
    let has_env_split = words.windows(2).any(
        |pair| matches!(pair, [left, right] if left.ends_with(ENV) && SPLIT_STRING_FLAGS.contains(right)),
    );
    let mut report = |rule| {
        findings.found.push(Finding {
            rule,
            byte: first.start_byte(),
            point: first.start_position(),
            subject: shebang.to_owned(),
            fix: None,
        });
    };
    if words.len() > MAX_SHEBANG_WORDS && !has_env_split {
        report(ShellRule::Sc2096);
    }
    if context.shell_specified {
        return;
    }
    if shebang_executable(&words) == ASH {
        report(ShellRule::Sc2187);
    }
    if shebang.is_empty() {
        return;
    }
    if !shebang.starts_with(PATH_SEPARATOR) {
        report(ShellRule::Sc2239);
    }
    if words.first().is_some_and(|word| word.ends_with(PATH_SEPARATOR)) {
        report(ShellRule::Sc2246);
    }
}
