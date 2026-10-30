use std::collections::HashMap;

use tree_sitter::Node;

use super::annotations::is_disabled;
use crate::rules::ShellRule;
use crate::shell::commands::Command;
use crate::shell::context::{Context, Findings};
use crate::shell::parameters::{modifier, reference};
use crate::shell::syntax::{EXPANSION, EXPANSIONS, FUNCTION, NAME_FIELD, WORD, ancestors, descendants, text};
use crate::shell::words::{braced_content, literal};

const SET: &str = "set";
const END_OF_OPTIONS: &str = "--";
const OPTION_PREFIX: char = '-';
const LONG_OPTION_FLAG: &str = "-o";
const ALL_PARAMETERS: &[&str] = &["*", "@", "#"];
const SCRIPT_NAME: &str = "0";
const DEFAULT_MODIFIERS: &[char] = &['-', '+', '?'];
const COLON: char = ':';
const UNPASSED_DECLARATION: &str = "sc2120";

struct Call<'tree> {
    node: Node<'tree>,
    without_arguments: bool,
}

// `checkUnpassedInFunctions`: una función que lee `$1`, `$@` o `$#` y a la que nunca se le pasan
// argumentos. Cada llamada sin argumentos avisa SC2119 y la definición SC2120.
pub fn check(context: &Context<'_, '_>, findings: &mut Findings) {
    let source = context.source;
    let nodes = descendants(context.root);
    let mut functions: HashMap<String, Node<'_>> = HashMap::new();
    for function in nodes.iter().filter(|node| node.kind() == FUNCTION) {
        if reads_positional_parameters(*function, source)
            && let Some(name) = function.child_by_field_name(NAME_FIELD)
        {
            functions.insert(text(name, source).to_owned(), *function);
        }
    }
    let mut calls: Vec<(String, Call<'_>)> = Vec::new();
    for command in nodes.iter().filter_map(|node| Command::of(*node)) {
        let name = command
            .first_word()
            .filter(|word| word.kind() == WORD)
            .and_then(|word| literal(word, source));
        if let Some(name) = name.filter(|name| functions.contains_key(name)) {
            calls.push((
                name,
                Call {
                    node: command.node,
                    without_arguments: command.arguments().is_empty(),
                },
            ));
        }
    }
    let mut names: Vec<&String> = functions.keys().collect();
    names.sort_by_key(|name| functions.get(*name).map(Node::start_byte));
    for name in names {
        let group: Vec<&Call<'_>> = calls
            .iter()
            .filter(|(called, _)| called == name)
            .map(|(_, call)| call)
            .collect();
        let Some(function) = functions.get(name) else {
            continue;
        };
        if group.is_empty()
            || !group.iter().all(|call| call.without_arguments)
            || is_disabled(*function, source, UNPASSED_DECLARATION)
        {
            continue;
        }
        for call in group {
            findings.report(ShellRule::Sc2119, call.node, name);
        }
        findings.report(ShellRule::Sc2120, *function, name);
    }
}

fn reads_positional_parameters(function: Node<'_>, source: &[u8]) -> bool {
    let inside = descendants(function);
    let reads = inside.iter().any(|node| {
        EXPANSIONS.contains(&node.kind())
            && nearest_function(*node) == Some(function)
            && is_positional_reference(*node, source)
    });
    reads
        && !inside
            .iter()
            .any(|node| sets_positional_parameters(*node, source))
}

fn nearest_function(node: Node<'_>) -> Option<Node<'_>> {
    ancestors(node).find(|ancestor| ancestor.kind() == FUNCTION)
}

fn is_positional_reference(expansion: Node<'_>, source: &[u8]) -> bool {
    let braced = braced_content(expansion, source);
    let name = reference(&braced);
    let positional = ALL_PARAMETERS.contains(&name.as_str())
        || (!name.is_empty() && name != SCRIPT_NAME && name.chars().all(|digit| digit.is_ascii_digit()));
    positional && !(expansion.kind() == EXPANSION && has_default_value(&braced))
}

// `${1-x}`, `${1:-x}`, `${1+x}` y `${1?x}` no fallan sin argumentos.
fn has_default_value(braced: &str) -> bool {
    let modifier = modifier(braced);
    let operator = modifier.strip_prefix(COLON).unwrap_or(&modifier);
    operator.starts_with(DEFAULT_MODIFIERS)
}

// `set` con `--` o con algún argumento que no es opción reemplaza los parámetros posicionales.
fn sets_positional_parameters(node: Node<'_>, source: &[u8]) -> bool {
    let Some(command) = Command::of(node) else {
        return false;
    };
    let is_set = command
        .first_word()
        .filter(|word| word.kind() == WORD)
        .and_then(|word| literal(word, source))
        .is_some_and(|name| name == SET);
    if !is_set {
        return false;
    }
    let mut arguments = command.arguments().iter();
    while let Some(argument) = arguments.next() {
        match literal(*argument, source).as_deref() {
            Some(LONG_OPTION_FLAG) => {
                arguments.next();
            }
            Some(END_OF_OPTIONS) => return true,
            Some(option) if option.starts_with(OPTION_PREFIX) => {}
            _ => return true,
        }
    }
    false
}
