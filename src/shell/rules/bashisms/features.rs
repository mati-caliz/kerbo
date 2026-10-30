use tree_sitter::Node;

use super::Found;
use crate::rules::ShellRule;
use crate::shell::context::{Context, Position};
use crate::shell::parameters::is_variable_name;
use crate::shell::syntax::{
    ANSI_STRING, ARITHMETIC_EXPANSION, ARITHMETIC_FOR, ARRAY, ASSIGNMENT, BINARY, COMMAND,
    COMMAND_SUBSTITUTION, CONCATENATION, DOUBLE_BRACKET, FOR, FUNCTION, HERESTRING, NAME_FIELD, NUMBER,
    OPERATOR_FIELD, PIPELINE, POSTFIX, PROCESS_SUBSTITUTION, REDIRECTED, STRING, TEST, UNARY, WORD,
    ancestors, children, first_token, is_arithmetic_command, named_children, text,
};

const EXTGLOB: &str = "extglob_pattern";
const TRANSLATED: &str = "translated_string";
const FILE_REDIRECT_KINDS: &[&str] = &["file_redirect", HERESTRING, "heredoc_redirect"];
const DOLLAR: &str = "$";
const OLD_ARITHMETIC: &str = "$[";
const SELECT: &str = "select";
const HERESTRING_OPERATOR: &str = "<<<";
const STEP_OPERATORS: &[&str] = &["++", "--"];
const EXPONENT: &str = "**";
const APPEND: &str = "+=";
const PIPE_STDERR: &str = "|&";
const COPROC: &str = "coproc";
const LET: &str = "let";
const BACKTICK: &str = "`";
const EXTGLOB_STARTS: &[&str] = &["?(", "*(", "+(", "@(", "!("];
const BASE_SEPARATOR: char = '#';
const OPEN_BRACE: u8 = b'{';
const CLOSE_BRACE: u8 = b'}';
const BRACE_LIST: u8 = b',';
const BRACE_RANGE: &[u8] = b"..";
const NEGATED_CLASS: &[u8] = b"[^";
const CLOSE_CLASS: u8 = b']';
const ESCAPE: u8 = b'\\';
const ARITHMETIC_CONTEXTS: &[&str] = &[ARITHMETIC_EXPANSION, ARITHMETIC_FOR, "subscript"];
const GLOB_FREE_ANCESTORS: &[&str] = &["expansion", ARITHMETIC_EXPANSION, "regex", TEST];

pub fn check(node: Node<'_>, context: &Context<'_, '_>, found: &mut Found<'_>) {
    let source = context.source;
    match node.kind() {
        PROCESS_SUBSTITUTION => found.report(ShellRule::Sc3001, node, "<(...)"),
        EXTGLOB => check_extglob(node, source, found),
        ANSI_STRING if !found.shell().is_busybox() => found.report(ShellRule::Sc3003, node, "$'...'"),
        TRANSLATED => found.report(ShellRule::Sc3004, node, "$\"...\""),
        ARITHMETIC_FOR => found.report(ShellRule::Sc3005, node, "for ((...))"),
        ARITHMETIC_EXPANSION if first_token(node, source) == OLD_ARITHMETIC => {
            found.report(ShellRule::Sc3007, node, OLD_ARITHMETIC);
        }
        FOR if first_token(node, source) == SELECT => found.report(ShellRule::Sc3008, node, SELECT),
        "brace_expression" => found.report(ShellRule::Sc3009, node, text(node, source)),
        CONCATENATION => check_concatenation(node, source, found),
        TEST if first_token(node, source) == DOUBLE_BRACKET && !found.shell().is_busybox() => {
            found.report(ShellRule::Sc3010, node, DOUBLE_BRACKET);
        }
        HERESTRING => check_herestring(node, source, found),
        POSTFIX | UNARY => check_step(node, source, found),
        BINARY => check_exponent(node, source, found),
        ASSIGNMENT => check_append(node, source, found),
        PIPELINE => check_pipe_stderr(node, source, found),
        ARRAY if node.parent().is_some_and(|parent| parent.kind() == ASSIGNMENT) => {
            found.report(ShellRule::Sc3030, node, "(...)");
        }
        FUNCTION => check_function_name(node, source, found),
        COMMAND_SUBSTITUTION => check_file_read(node, source, found),
        NUMBER => check_base(node, source, found),
        COMMAND => check_command(node, source, found),
        WORD => check_word(node, source, found),
        _ if node.kind() == DOLLAR && !node.is_named() => check_translated(node, found),
        _ => {}
    }
    if node.kind() == "compound_statement" && is_arithmetic_command(node, source) {
        found.report(ShellRule::Sc3006, node, "((...))");
    }
}

fn check_extglob(node: Node<'_>, source: &[u8], found: &mut Found<'_>) {
    let pattern = text(node, source);
    if let Some(offset) = EXTGLOB_STARTS
        .iter()
        .filter_map(|start| pattern.find(start))
        .min()
    {
        found.report_at(ShellRule::Sc3002, Position::of(node).shifted(offset), pattern);
    }
    report_negated_classes(node, source, found);
}

fn check_translated(dollar: Node<'_>, found: &mut Found<'_>) {
    let joined_string = dollar
        .next_sibling()
        .is_some_and(|next| next.kind() == STRING && next.start_byte() == dollar.end_byte());
    let quoted = dollar.parent().is_some_and(|parent| parent.kind() == STRING);
    if joined_string && !quoted {
        found.report(ShellRule::Sc3004, dollar, "$\"...\"");
    }
}

fn check_concatenation(node: Node<'_>, source: &[u8], found: &mut Found<'_>) {
    if ancestors(node).any(|ancestor| ancestor.kind() == CONCATENATION) {
        return;
    }
    let unquoted = unquoted_text(node, source);
    for offset in brace_expansions(&unquoted.text) {
        found.report_at(ShellRule::Sc3009, unquoted.position(node, offset), "{...}");
    }
    report_negated_classes(node, source, found);
}

fn check_word(node: Node<'_>, source: &[u8], found: &mut Found<'_>) {
    let in_concatenation = node.parent().is_some_and(|parent| parent.kind() == CONCATENATION);
    if !in_concatenation {
        report_negated_classes(node, source, found);
    }
}

// Las partes sin comillas de una palabra, pegadas, con el byte de cada una en el archivo.
struct Unquoted {
    text: Vec<u8>,
    starts: Vec<usize>,
}

impl Unquoted {
    fn position(&self, node: Node<'_>, offset: usize) -> Position {
        let byte = self
            .starts
            .get(offset)
            .copied()
            .unwrap_or_else(|| node.start_byte());
        Position::of(node).shifted(byte.saturating_sub(node.start_byte()))
    }
}

fn unquoted_text(node: Node<'_>, source: &[u8]) -> Unquoted {
    let mut unquoted = Unquoted {
        text: Vec::new(),
        starts: Vec::new(),
    };
    let parts = if node.kind() == CONCATENATION {
        children(node)
    } else {
        vec![node]
    };
    for part in parts {
        let quoted = part.kind() != WORD && part.kind() != EXTGLOB;
        let bytes = source.get(part.start_byte()..part.end_byte()).unwrap_or_default();
        for (index, byte) in bytes.iter().enumerate() {
            unquoted.text.push(if quoted { 0 } else { *byte });
            unquoted.starts.push(part.start_byte().saturating_add(index));
        }
    }
    unquoted
}

// Los `{` que abren una expansión de llaves de bash: una lista con coma o un rango con `..`.
fn brace_expansions(text: &[u8]) -> Vec<usize> {
    let mut openings = Vec::new();
    let mut index = 0;
    while index < text.len() {
        if text.get(index) == Some(&ESCAPE) {
            index = index.saturating_add(2);
            continue;
        }
        if text.get(index) == Some(&OPEN_BRACE)
            && let Some(length) = text
                .get(index.saturating_add(1)..)
                .and_then(|rest| rest.iter().position(|byte| *byte == CLOSE_BRACE))
        {
            let inside = text.get(index.saturating_add(1)..index.saturating_add(1).saturating_add(length));
            if inside.is_some_and(is_brace_content) {
                openings.push(index);
            }
        }
        index = index.saturating_add(1);
    }
    openings
}

fn is_brace_content(inside: &[u8]) -> bool {
    !inside.contains(&0)
        && !inside.contains(&OPEN_BRACE)
        && (inside.contains(&BRACE_LIST) || inside.windows(BRACE_RANGE.len()).any(|pair| pair == BRACE_RANGE))
}

fn report_negated_classes(node: Node<'_>, source: &[u8], found: &mut Found<'_>) {
    if ancestors(node).any(|ancestor| GLOB_FREE_ANCESTORS.contains(&ancestor.kind())) {
        return;
    }
    let unquoted = unquoted_text(node, source);
    let text = &unquoted.text;
    for index in 0..text.len() {
        let escaped = index > 0 && text.get(index.saturating_sub(1)) == Some(&ESCAPE);
        let closes = text
            .get(index.saturating_add(NEGATED_CLASS.len())..)
            .is_some_and(|rest| rest.contains(&CLOSE_CLASS));
        if !escaped
            && text
                .get(index..)
                .is_some_and(|rest| rest.starts_with(NEGATED_CLASS))
            && closes
        {
            found.report_at(ShellRule::Sc3026, unquoted.position(node, index), "[^...]");
        }
    }
}

fn check_herestring(node: Node<'_>, source: &[u8], found: &mut Found<'_>) {
    if let Some(operator) = children(node)
        .into_iter()
        .find(|child| text(*child, source) == HERESTRING_OPERATOR)
    {
        found.report(ShellRule::Sc3011, operator, HERESTRING_OPERATOR);
    }
}

fn check_step(node: Node<'_>, source: &[u8], found: &mut Found<'_>) {
    if let Some(operator) = node.child_by_field_name(OPERATOR_FIELD)
        && STEP_OPERATORS.contains(&text(operator, source))
    {
        found.report(ShellRule::Sc3018, operator, text(operator, source));
    }
}

fn check_exponent(node: Node<'_>, source: &[u8], found: &mut Found<'_>) {
    if let Some(operator) = node.child_by_field_name(OPERATOR_FIELD)
        && text(operator, source) == EXPONENT
    {
        found.report(ShellRule::Sc3019, operator, EXPONENT);
    }
}

fn check_append(node: Node<'_>, source: &[u8], found: &mut Found<'_>) {
    if children(node).iter().any(|child| text(*child, source) == APPEND) {
        found.report(ShellRule::Sc3024, node, APPEND);
    }
}

fn check_pipe_stderr(node: Node<'_>, source: &[u8], found: &mut Found<'_>) {
    for operator in children(node)
        .into_iter()
        .filter(|child| text(*child, source) == PIPE_STDERR)
    {
        found.report(ShellRule::Sc3029, operator, PIPE_STDERR);
    }
}

fn check_function_name(node: Node<'_>, source: &[u8], found: &mut Found<'_>) {
    if let Some(name) = node.child_by_field_name(NAME_FIELD)
        && !is_variable_name(text(name, source))
    {
        found.report(ShellRule::Sc3033, node, text(name, source));
    }
}

fn check_file_read(node: Node<'_>, source: &[u8], found: &mut Found<'_>) {
    let parts = named_children(node);
    let only_redirections = match parts.as_slice() {
        [redirected] if redirected.kind() == REDIRECTED => named_children(*redirected)
            .iter()
            .all(|child| FILE_REDIRECT_KINDS.contains(&child.kind())),
        [] => false,
        redirects => redirects
            .iter()
            .all(|child| FILE_REDIRECT_KINDS.contains(&child.kind())),
    };
    if !only_redirections {
        return;
    }
    if first_token(node, source) == BACKTICK {
        found.report(ShellRule::Sc3035, node, "`<file`");
    } else {
        found.report(ShellRule::Sc3034, node, "$(<file)");
    }
}

fn check_base(node: Node<'_>, source: &[u8], found: &mut Found<'_>) {
    let in_arithmetic = ancestors(node).any(|ancestor| {
        ARITHMETIC_CONTEXTS.contains(&ancestor.kind()) || is_arithmetic_command(ancestor, source)
    });
    let number = text(node, source);
    let digits = number.split(BASE_SEPARATOR).next().unwrap_or_default();
    if in_arithmetic
        && number.contains(BASE_SEPARATOR)
        && !digits.is_empty()
        && digits.chars().all(|digit| digit.is_ascii_digit())
    {
        found.report(ShellRule::Sc3052, node, number);
    }
}

fn check_command(node: Node<'_>, source: &[u8], found: &mut Found<'_>) {
    let Some(name) = node.child_by_field_name(NAME_FIELD) else {
        return;
    };
    match text(name, source) {
        COPROC => found.report(ShellRule::Sc3032, node, COPROC),
        LET => check_let_arithmetic(node, source, found),
        _ => {}
    }
}

// `let` lee sus argumentos como aritmética: ShellCheck los parsea y ve sus `++`, `--` y `**`.
fn check_let_arithmetic(node: Node<'_>, source: &[u8], found: &mut Found<'_>) {
    for argument in named_children(node)
        .into_iter()
        .skip(1)
        .filter(|argument| argument.kind() == WORD)
    {
        let expression = text(argument, source);
        for operator in STEP_OPERATORS {
            for (offset, _) in expression.match_indices(operator) {
                found.report_at(
                    ShellRule::Sc3018,
                    Position::of(argument).shifted(offset),
                    operator,
                );
            }
        }
        for (offset, _) in expression.match_indices(EXPONENT) {
            found.report_at(
                ShellRule::Sc3019,
                Position::of(argument).shifted(offset),
                EXPONENT,
            );
        }
    }
}
