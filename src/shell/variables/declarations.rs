use tree_sitter::Node;

use super::arithmetic::let_events;
use super::{Flow, assigned_name, variables_in_literal};
use crate::shell::commands::{Command, bsd_options, generic_options, gnu_options};
use crate::shell::context::Position;
use crate::shell::parameters::{is_variable_name, reference};
use crate::shell::syntax::{
    ASSIGNMENT, BINARY, LEFT_FIELD, OPERATOR_FIELD, RIGHT_FIELD, TEST, UNARY, VARIABLE_NAME, children,
    is_double_bracket, named_children, text,
};
use crate::shell::words::{literal, literal_or};

const READ_OPTIONS: &str = "sreu:n:N:i:p:a:t:";
const MAPFILE_OPTIONS: &str = "d:n:O:s:u:C:c:t";
const ARRAY_FLAG: &str = "a";
const FLAG_PREFIX: char = '-';
const INDEX_START: char = '[';
const DEFAULT_MAPFILE: &str = "MAPFILE";
const FLAGS_PREFIX: &str = "FLAGS_";
const NOT_LITERAL: &str = "\0";
const NOT_LITERAL_SPACE: &str = " ";
const NOT_LITERAL_BANG: &str = "!";
const DEREFERENCING_OPERATORS: &[&str] = &["-eq", "-ne", "-lt", "-le", "-gt", "-ge"];
const TEST_VARIABLE_OPERATORS: &[&str] = &["-v", "-R"];
const DEFINE_COMMANDS: &[&str] = &[
    "DEFINE_boolean",
    "DEFINE_float",
    "DEFINE_integer",
    "DEFINE_string",
];
const FUNCTION_FLAGS: &[&str] = &["f", "F"];
const EXPORTING_FLAGS: &[&str] = &["x", "p"];
const PRINT_FLAG: &str = "p";
const EXPORT_FLAG: &str = "x";
const FUNCTION_FLAG: &str = "f";

pub fn references(node: Node<'_>, source: &[u8], flow: &mut Flow) {
    match node.kind() {
        UNARY if inside_test(node) => {
            let operator = node
                .child_by_field_name(OPERATOR_FIELD)
                .map(|operator| text(operator, source));
            if operator.is_some_and(|operator| TEST_VARIABLE_OPERATORS.contains(&operator))
                && let Some(operand) = named_children(node).last()
            {
                test_variable_reference(*operand, source, flow);
            }
        }
        BINARY
            if node
                .parent()
                .is_some_and(|parent| is_double_bracket(parent, source) || inside_test(parent)) =>
        {
            let operator = node
                .child_by_field_name(OPERATOR_FIELD)
                .map(|operator| text(operator, source));
            if operator.is_some_and(|operator| DEREFERENCING_OPERATORS.contains(&operator))
                && enclosing_test(node).is_some_and(|test| is_double_bracket(test, source))
            {
                [LEFT_FIELD, RIGHT_FIELD]
                    .into_iter()
                    .filter_map(|side| node.child_by_field_name(side))
                    .for_each(|operand| test_variable_reference(operand, source, flow));
            }
        }
        _ => {
            if let Some(command) = Command::of(node) {
                command_references(&command, source, flow);
            }
        }
    }
}

fn inside_test(node: Node<'_>) -> bool {
    enclosing_test(node).is_some()
}

fn enclosing_test(node: Node<'_>) -> Option<Node<'_>> {
    crate::shell::syntax::ancestors(node).find(|ancestor| ancestor.kind() == TEST)
}

fn test_variable_reference(operand: Node<'_>, source: &[u8], flow: &mut Flow) {
    let raw = literal_or(operand, source, NOT_LITERAL);
    let name: String = raw
        .chars()
        .take_while(|character| *character != INDEX_START)
        .collect();
    if is_variable_name(&name) {
        flow.refer(reference(&name), Position::of(operand));
    }
}

fn command_references(command: &Command<'_>, source: &[u8], flow: &mut Flow) {
    let Some(name) = command.literal_name(source) else {
        return;
    };
    let flags: Vec<String> = command.flags(source).into_iter().map(|(_, flag)| flag).collect();
    let has = |wanted: &[&str]| flags.iter().any(|flag| wanted.contains(&flag.as_str()));
    let refers_arguments = match name.as_str() {
        "declare" | "typeset" => has(EXPORTING_FLAGS) && !has(FUNCTION_FLAGS),
        "export" => !has(&[FUNCTION_FLAG]),
        "local" => has(&[EXPORT_FLAG]),
        _ => false,
    };
    if refers_arguments {
        for argument in command.arguments() {
            if let Some(referenced) = declared_name(*argument, source) {
                flow.refer(referenced, Position::of(*argument));
            }
        }
    }
    match name.as_str() {
        "trap" => {
            if let Some(first) = command.arguments().first() {
                for referenced in variables_in_literal(&literal_or(*first, source, NOT_LITERAL_SPACE)) {
                    flow.refer(referenced, Position::of(command.node));
                }
            }
        }
        "alias" => {
            for argument in command.arguments() {
                for referenced in variables_in_literal(&literal_or(*argument, source, NOT_LITERAL_SPACE)) {
                    flow.refer(referenced, Position::of(*argument));
                }
            }
        }
        _ => {}
    }
}

fn declared_name(argument: Node<'_>, source: &[u8]) -> Option<String> {
    if argument.kind() == ASSIGNMENT {
        return assigned_name(argument, source);
    }
    let name = if argument.kind() == VARIABLE_NAME {
        text(argument, source).to_owned()
    } else {
        literal_word(argument, source)?
    };
    (!name.starts_with(FLAG_PREFIX)).then_some(name)
}

fn literal_word(argument: Node<'_>, source: &[u8]) -> Option<String> {
    (children(argument).is_empty() || argument.kind() != crate::shell::syntax::CONCATENATION)
        .then(|| literal(argument, source))
        .flatten()
}

pub fn modifications(command: &Command<'_>, source: &[u8], flow: &mut Flow) {
    let Some(name) = command.literal_name(source) else {
        return;
    };
    let arguments = command.arguments();
    let flags: Vec<String> = command.flags(source).into_iter().map(|(_, flag)| flag).collect();
    let has = |wanted: &[&str]| flags.iter().any(|flag| wanted.contains(&flag.as_str()));
    match name.as_str() {
        "builtin" => {
            if let Some(rest) = command_after_builtin(command) {
                modifications(&rest, source, flow);
            }
        }
        "read" => read_modifications(arguments, source, flow),
        "getopts" => {
            if let [_, variable, ..] = arguments {
                literal_assignment(*variable, source, flow);
            }
        }
        "let" => {
            for argument in arguments {
                let_events(*argument, source, flow);
            }
        }
        "local" => declared_modifications(arguments, source, flow),
        "declare" | "typeset" if !has(FUNCTION_FLAGS) && !has(&[PRINT_FLAG]) => {
            declared_modifications(arguments, source, flow);
        }
        "readonly" if !has(&[FUNCTION_FLAG, PRINT_FLAG]) => declared_modifications(arguments, source, flow),
        "printf" => flag_assignment(bsd_options("v:", arguments, source), "v", source, flow),
        "wait" => flag_assignment(Some(generic_options(arguments, source)), "p", source, flow),
        "mapfile" | "readarray" => mapfile_modifications(command, source, flow),
        define if DEFINE_COMMANDS.contains(&define) => {
            if let [flag_name, _, ..] = arguments
                && let Some(flag) = literal(*flag_name, source)
            {
                flow.assign(format!("{FLAGS_PREFIX}{flag}"), Position::of(*flag_name));
            }
        }
        _ => {}
    }
}

fn command_after_builtin<'tree>(command: &Command<'tree>) -> Option<Command<'tree>> {
    let words = command.words.get(1..)?.to_vec();
    (!words.is_empty()).then(|| Command {
        node: command.node,
        words,
        assignments: Vec::new(),
    })
}

fn read_modifications(arguments: &[Node<'_>], source: &[u8], flow: &mut Flow) {
    let Some(options) = gnu_options(READ_OPTIONS, arguments, source) else {
        let trailing: Vec<Node<'_>> = arguments
            .iter()
            .rev()
            .take_while(|argument| plain_literal(**argument, source).is_some())
            .copied()
            .collect();
        for argument in trailing.into_iter().rev() {
            literal_assignment(argument, source, flow);
        }
        return;
    };
    if let Some((_, option)) = options.iter().find(|(flag, _)| flag == ARRAY_FLAG) {
        literal_assignment(option.value, source, flow);
        return;
    }
    for (_, option) in options.iter().filter(|(flag, _)| flag.is_empty()) {
        literal_assignment(option.value, source, flow);
    }
}

fn plain_literal(argument: Node<'_>, source: &[u8]) -> Option<String> {
    literal(argument, source).filter(|value| !value.starts_with(FLAG_PREFIX))
}

fn literal_assignment(argument: Node<'_>, source: &[u8], flow: &mut Flow) {
    if let Some(name) = plain_literal(argument, source) {
        flow.assign(name, Position::of(argument));
    }
}

fn declared_modifications(arguments: &[Node<'_>], source: &[u8], flow: &mut Flow) {
    for argument in arguments {
        if argument.kind() == ASSIGNMENT {
            if let Some(name) = assigned_name(*argument, source) {
                flow.assign(name, Position::of(*argument));
            }
            continue;
        }
        let name = if argument.kind() == VARIABLE_NAME {
            Some(text(*argument, source).to_owned())
        } else {
            literal(*argument, source)
        };
        if let Some(name) = name.filter(|name| is_variable_name(name)) {
            flow.assign(name, Position::of(*argument));
        }
    }
}

fn flag_assignment(
    options: Option<Vec<(String, crate::shell::commands::Opt<'_>)>>,
    wanted: &str,
    source: &[u8],
    flow: &mut Flow,
) {
    let Some(options) = options else {
        return;
    };
    let Some((_, option)) = options.iter().find(|(flag, _)| flag == wanted) else {
        return;
    };
    let variable = literal_or(option.value, source, NOT_LITERAL_BANG);
    let base: String = variable
        .chars()
        .take_while(|character| *character != INDEX_START)
        .collect();
    if !base.starts_with(FLAG_PREFIX) {
        flow.assign(base, Position::of(option.value));
    }
}

fn mapfile_modifications(command: &Command<'_>, source: &[u8], flow: &mut Flow) {
    let arguments = command.arguments();
    if let Some(options) = gnu_options(MAPFILE_OPTIONS, arguments, source) {
        match options.iter().find(|(flag, _)| flag.is_empty()) {
            None => flow.assign(DEFAULT_MAPFILE, Position::of(command.node)),
            Some((_, option)) => {
                if let Some(name) = literal(option.value, source).filter(|name| is_variable_name(name)) {
                    flow.assign(name, Position::of(option.value));
                }
            }
        }
        return;
    }
    let fallback = arguments.iter().rev().find_map(|argument| {
        literal(*argument, source)
            .filter(|name| is_variable_name(name))
            .map(|name| (name, *argument))
    });
    if let Some((name, argument)) = fallback {
        flow.assign(name, Position::of(argument));
    }
}
