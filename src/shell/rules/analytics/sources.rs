use tree_sitter::Node;

use super::annotations::source_override;
use crate::rules::ShellRule;
use crate::shell::commands::{Command, merged_commands};
use crate::shell::context::{Context, Findings};
use crate::shell::syntax::{
    ARITHMETIC_EXPANSION, COMMAND_SUBSTITUTION, EXPANSION, SIMPLE_EXPANSION, WORD, text,
};
use crate::shell::words::{literal, parts};

const SOURCE_COMMANDS: &[&str] = &["source", "."];
const LINE_BREAKS: &[char] = &['\n', '\r'];
const END_OF_OPTIONS: &str = "--";
const PATH_OPTION: &str = "-p";
const NULL_DEVICE: &str = "/dev/null";
const HOME_PREFIX: &str = "~/";
const DIRECTORY_SEPARATOR: &str = "/";
const CURRENT_DIRECTORY: &str = ".";
const STRING_EXPANSIONS: &[&str] = &[
    SIMPLE_EXPANSION,
    EXPANSION,
    COMMAND_SUBSTITUTION,
    ARITHMETIC_EXPANSION,
];

// `readSource` del parser de ShellCheck: sin `-x` no sigue ningún `source`, así que avisa SC1091
// por cada uno con un archivo que puede nombrar y SC1090 por los que no.
pub fn check(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let Some(command) = Command::of(node) else {
        return;
    };
    let source = context.source;
    for words in merged_commands(&command.words, source) {
        check_source(node, words, source, findings);
    }
}

fn check_source(node: Node<'_>, words: &[Node<'_>], source: &[u8], findings: &mut Findings) {
    let Some((name_word, arguments)) = words.split_first() else {
        return;
    };
    let name_word = *name_word;
    if name_word.kind() != WORD {
        return;
    }
    let is_source = literal(name_word, source)
        .is_some_and(|name| SOURCE_COMMANDS.contains(&name.trim_start_matches(LINE_BREAKS)));
    if !is_source {
        return;
    }
    let file = sourced_file(arguments, source);
    let position = file.unwrap_or(name_word);
    let name = source_override(node, source)
        .or_else(|| file.and_then(|word| literal(word, source)))
        .or_else(|| file.and_then(|word| strip_dynamic_prefix(word, source)))
        .filter(|name| !name.starts_with(HOME_PREFIX));
    match name {
        None => findings.report(ShellRule::Sc1090, position, text(position, source)),
        Some(name) if name == NULL_DEVICE => {}
        Some(name) => findings.report(ShellRule::Sc1091, position, &name),
    }
}

fn sourced_file<'tree>(arguments: &[Node<'tree>], source: &[u8]) -> Option<Node<'tree>> {
    let (first, rest) = arguments.split_first()?;
    match literal(*first, source).as_deref() {
        Some(END_OF_OPTIONS) => rest.first().copied(),
        Some(PATH_OPTION) => rest.get(1).copied(),
        _ => Some(*first),
    }
}

// `$dir/lib.sh` se sigue como `./lib.sh`: una expansión al principio y el resto literal con `/`.
fn strip_dynamic_prefix(word: Node<'_>, source: &[u8]) -> Option<String> {
    let pieces = parts(word);
    let (first, rest) = pieces.split_first()?;
    if !STRING_EXPANSIONS.contains(&first.kind()) {
        return None;
    }
    let tail: String = rest
        .iter()
        .map(|piece| literal(*piece, source))
        .collect::<Option<String>>()?;
    tail.starts_with(DIRECTORY_SEPARATOR)
        .then(|| format!("{CURRENT_DIRECTORY}{tail}"))
}
