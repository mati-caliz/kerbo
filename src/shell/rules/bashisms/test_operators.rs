use tree_sitter::Node;

use super::{Found, Posix};
use crate::rules::ShellRule;
use crate::shell::commands::Command;
use crate::shell::context::Context;
use crate::shell::syntax::{BINARY, OPERATOR_FIELD, TEST, UNARY, ancestors, text};
use crate::shell::words::literal;

const TEST_COMMAND: &str = "test";
const UNARY_WORDS: usize = 3;
const BINARY_WORDS: usize = 4;
const LEXICOGRAPHIC: &[&str] = &["<", ">", "\\<", "\\>", "<=", ">=", "\\<=", "\\>="];
const DOUBLE_EQUALS: &str = "==";
const REGEX_MATCH: &str = "=~";
const ARITHMETIC_KINDS: &[&str] = &[
    "arithmetic_expansion",
    "c_style_for_statement",
    "compound_statement",
    "subscript",
];

// Los dialectos que sí tienen un operador de `test` que sh no define.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Supported {
    Nowhere,
    InBusybox,
    InDashAndBusybox,
}

// Cada operador de `test` que sh no tiene, con su código.
#[derive(Debug, Clone, Copy)]
struct Operator {
    rule: ShellRule,
    supported: Supported,
}

const fn operator(rule: ShellRule, supported: Supported) -> Operator {
    Operator { rule, supported }
}

fn binary_operator(text: &str) -> Option<Operator> {
    if LEXICOGRAPHIC.contains(&text) {
        return Some(operator(ShellRule::Sc3012, Supported::InDashAndBusybox));
    }
    match text {
        DOUBLE_EQUALS => Some(operator(ShellRule::Sc3014, Supported::InBusybox)),
        REGEX_MATCH => Some(operator(ShellRule::Sc3015, Supported::Nowhere)),
        _ => None,
    }
}

const UNARY_OPERATORS: &[(&str, Operator)] = &[
    ("-v", operator(ShellRule::Sc3016, Supported::Nowhere)),
    ("-a", operator(ShellRule::Sc3017, Supported::Nowhere)),
    ("-o", operator(ShellRule::Sc3062, Supported::Nowhere)),
    ("-R", operator(ShellRule::Sc3063, Supported::Nowhere)),
    ("-N", operator(ShellRule::Sc3064, Supported::Nowhere)),
    ("-k", operator(ShellRule::Sc3065, Supported::InDashAndBusybox)),
    ("-G", operator(ShellRule::Sc3066, Supported::InDashAndBusybox)),
    ("-O", operator(ShellRule::Sc3067, Supported::InDashAndBusybox)),
];

fn unary_operator(text: &str) -> Option<Operator> {
    UNARY_OPERATORS
        .iter()
        .find(|(name, _)| *name == text)
        .map(|(_, operator)| *operator)
}

impl Operator {
    const fn applies(self, shell: Posix) -> bool {
        match shell {
            Posix::Sh => true,
            Posix::Dash => !matches!(self.supported, Supported::InDashAndBusybox),
            Posix::Busybox => matches!(self.supported, Supported::Nowhere),
        }
    }
}

pub fn check(node: Node<'_>, context: &Context<'_, '_>, found: &mut Found<'_>) {
    let source = context.source;
    match node.kind() {
        BINARY | UNARY if is_test_operand(node) => {
            let Some(operator_node) = node.child_by_field_name(OPERATOR_FIELD) else {
                return;
            };
            let operator_text = text(operator_node, source);
            let operator = if node.kind() == BINARY {
                binary_operator(operator_text)
            } else {
                unary_operator(operator_text)
            };
            if let Some(operator) = operator.filter(|operator| operator.applies(found.shell())) {
                found.report(operator.rule, operator_node, operator_text);
            }
        }
        _ => check_test_command(node, source, found),
    }
}

fn is_test_operand(node: Node<'_>) -> bool {
    ancestors(node)
        .find(|ancestor| ancestor.kind() == TEST || ARITHMETIC_KINDS.contains(&ancestor.kind()))
        .is_some_and(|ancestor| ancestor.kind() == TEST)
}

// tree-sitter deja `==` y `=~` sueltos como tokens sin nombre: su texto es el operador.
fn word_text(word: Node<'_>, source: &[u8]) -> Option<String> {
    if word.is_named() {
        literal(word, source)
    } else {
        Some(text(word, source).to_owned())
    }
}

// `test a == b` y `test -v x`: ShellCheck sólo los mira con exactamente esas palabras.
fn check_test_command(node: Node<'_>, source: &[u8], found: &mut Found<'_>) {
    let Some(command) = Command::of(node) else {
        return;
    };
    if command
        .first_word()
        .and_then(|word| literal(word, source))
        .as_deref()
        != Some(TEST_COMMAND)
    {
        return;
    }
    let words = &command.words;
    let operator = match words.len() {
        UNARY_WORDS => words
            .get(1)
            .and_then(|word| word_text(*word, source))
            .and_then(|text| unary_operator(&text).map(|operator| (operator, text))),
        BINARY_WORDS => words
            .get(2)
            .and_then(|word| word_text(*word, source))
            .and_then(|text| binary_operator(&text).map(|operator| (operator, text))),
        _ => None,
    };
    if let Some((operator, text)) = operator.filter(|(operator, _)| operator.applies(found.shell())) {
        found.report(operator.rule, node, &text);
    }
}
