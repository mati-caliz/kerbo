use std::collections::{BTreeSet, HashMap};

use tree_sitter::Node;

use crate::rules::ShellRule;
use crate::shell::commands::Command;
use crate::shell::context::{Context, Findings, Position};
use crate::shell::syntax::{FUNCTION, NAME_FIELD, descendants, text};
use crate::shell::word_pieces::{only_literal_text, pieces, unquoted_literal};

const ALIAS: &str = "alias";
const ALIAS_SEPARATOR: char = '=';
const FLAG_PREFIX: char = '-';
const PATH_SEPARATOR: char = '/';
const FIRST_NON_FLAG_COMMANDS: &[&str] = &["chroot", "screen", "sudo", "doas", "run0", "xargs", "tmux"];
const SSH: &str = "ssh";
const FIND: &str = "find";
const FIND_EXEC_FLAGS: &[&str] = &["-exec", "-execdir", "-ok"];

// SC2032 y SC2033: `checkFunctionsUsedExternally`.
pub fn check(context: &Context<'_, '_>, findings: &mut Findings) {
    let source = context.source;
    let nodes = descendants(context.root);
    let definitions = function_and_alias_definitions(&nodes, source);
    if definitions.is_empty() {
        return;
    }
    let mut definitions_reported = BTreeSet::new();
    for command in nodes.iter().filter_map(|node| Command::of(*node)) {
        let Some(name) = command.name(source) else {
            continue;
        };
        let basename = name.rsplit(PATH_SEPARATOR).next().unwrap_or(&name);
        let Some(name_word) = command.name_word(source) else {
            continue;
        };
        let arguments: Vec<Node<'_>> = command
            .words
            .iter()
            .skip_while(|word| **word != name_word)
            .skip(1)
            .copied()
            .collect();
        for candidate in potential_commands(basename, &arguments, source) {
            let literal = unquoted_literal(&pieces(candidate, source));
            let Some(definition) = literal.and_then(|literal| definitions.get(&literal)) else {
                continue;
            };
            findings.report(ShellRule::Sc2033, candidate, text(candidate, source));
            if definitions_reported.insert(definition.byte) {
                findings.report_at(ShellRule::Sc2032, *definition, basename);
            }
        }
    }
}

// `Map.union (functions t) (aliases t)`: si hay varias definiciones con el nombre, gana la primera del
// recorrido al revés de ShellCheck, o sea la primera del archivo; una función le gana a un alias.
fn function_and_alias_definitions(nodes: &[Node<'_>], source: &[u8]) -> HashMap<String, Position> {
    let mut definitions = HashMap::new();
    for node in nodes {
        if node.kind() == FUNCTION
            && let Some(name) = node.child_by_field_name(NAME_FIELD)
        {
            definitions
                .entry(text(name, source).to_owned())
                .or_insert_with(|| Position::of(*node));
        }
    }
    for command in nodes.iter().filter_map(|node| Command::of(*node)) {
        if command.literal_name(source).as_deref() != Some(ALIAS) {
            continue;
        }
        for argument in command.arguments() {
            let raw = only_literal_text(&pieces(*argument, source), "");
            if let Some((name, _)) = raw.split_once(ALIAS_SEPARATOR) {
                definitions
                    .entry(name.to_owned())
                    .or_insert_with(|| Position::of(*argument));
            }
        }
    }
    definitions
}

fn potential_commands<'tree>(name: &str, arguments: &[Node<'tree>], source: &[u8]) -> Vec<Node<'tree>> {
    let literal = |word: Node<'_>| only_literal_text(&pieces(word, source), "");
    let without_flags = || {
        arguments
            .iter()
            .copied()
            .skip_while(|word| literal(*word).starts_with(FLAG_PREFIX))
    };
    if FIRST_NON_FLAG_COMMANDS.contains(&name) {
        without_flags().take(1).collect()
    } else if name == SSH {
        without_flags().skip(1).take(1).collect()
    } else if name == FIND {
        arguments
            .iter()
            .copied()
            .skip_while(|word| !FIND_EXEC_FLAGS.contains(&literal(*word).as_str()))
            .skip(1)
            .take(1)
            .collect()
    } else {
        Vec::new()
    }
}
