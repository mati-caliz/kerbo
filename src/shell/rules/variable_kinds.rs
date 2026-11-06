use std::collections::{BTreeSet, HashMap, HashSet};

use tree_sitter::Node;

use crate::rules::ShellRule;
use crate::shell::commands::{Command, gnu_options};
use crate::shell::context::{Context, Findings, Position};
use crate::shell::quoting::{Strictness, is_quote_free};
use crate::shell::syntax::{
    ARRAY, ASSIGNMENT, COMMAND_SUBSTITUTION, DECLARATION, EXPANSIONS, NAME_FIELD, SUBSCRIPT, VALUE_FIELD,
    ancestors, children, descendants, text,
};
use crate::shell::variables::{Event, EventKind, flow};
use crate::shell::word_pieces::{Piece, PieceKind, pieces, simplified_text};
use crate::shell::words::braced_content;

const ARRAY_VARIABLES: &[&str] = &[
    "BASH_ALIASES",
    "BASH_ARGC",
    "BASH_ARGV",
    "BASH_CMDS",
    "BASH_LINENO",
    "BASH_REMATCH",
    "BASH_SOURCE",
    "BASH_VERSINFO",
    "COMP_WORDS",
    "COPROC",
    "DIRSTACK",
    "FUNCNAME",
    "GROUPS",
    "MAPFILE",
    "PIPESTATUS",
    "COMPREPLY",
];
const APPEND: &str = "+=";
const READ: &str = "read";
const READ_ARRAY_FLAG: &str = "a";
const READ_OPTIONS: &str = "sreu:n:N:i:p:a:t:d:";
const MAPFILE_COMMANDS: &[&str] = &["mapfile", "readarray"];
const DECLARING_COMMANDS: &[&str] = &["declare", "local", "typeset", "readonly", "export"];
const ARRAY_FLAGS: &[&str] = &["a", "A"];
const EVAL: &str = "eval";
const LENGTH_PREFIX: char = '#';
const DOUBLE_QUOTE: char = '"';
const SINGLE_QUOTE: char = '\'';
const ESCAPED_SPACE: &str = "\\ ";
const BEFORE_QUOTE: &[char] = &['/', '=', ' '];

// Los nodos de un archivo por el byte donde arrancan, para volver de un evento del flujo a su código.
struct Nodes<'tree> {
    assignments: HashMap<usize, Node<'tree>>,
    expansions: HashMap<usize, Node<'tree>>,
}

impl<'tree> Nodes<'tree> {
    fn of(root: Node<'tree>) -> Self {
        let mut nodes = Self {
            assignments: HashMap::new(),
            expansions: HashMap::new(),
        };
        for node in descendants(root) {
            if node.kind() == ASSIGNMENT {
                nodes.assignments.insert(node.start_byte(), node);
            } else if EXPANSIONS.contains(&node.kind()) {
                nodes.expansions.insert(node.start_byte(), node);
            }
        }
        nodes
    }
}

// SC2128, SC2178, SC2179, SC2089 y SC2090: `checkArrayWithoutIndex` y `checkQuotesInLiterals`, que
// recorren el flujo de variables en orden y sin mirar ramas.
pub fn check(context: &Context<'_, '_>, findings: &mut Findings) {
    let events = flow(context).events;
    let nodes = Nodes::of(context.root);
    check_arrays(&events, &nodes, context, findings);
    check_quotes(&events, &nodes, context, findings);
}

fn is_array_write(event: &Event, nodes: &Nodes<'_>, context: &Context<'_, '_>) -> bool {
    let source = context.source;
    if let Some(assignment) = nodes.assignments.get(&event.position.byte) {
        let has_array_value = assignment
            .child_by_field_name(VALUE_FIELD)
            .is_some_and(|value| value.kind() == ARRAY);
        return has_array_value
            || is_indexed(*assignment)
            || assignment
                .parent()
                .and_then(Command::of)
                .is_some_and(|command| declares_array(&command, source));
    }
    let at = context
        .root
        .descendant_for_byte_range(event.position.byte, event.position.byte);
    let command = at.and_then(|node| {
        std::iter::once(node)
            .chain(ancestors(node))
            .take_while(|ancestor| ancestor.kind() != COMMAND_SUBSTITUTION)
            .find_map(Command::of)
    });
    command.is_some_and(|command| {
        let name = command.literal_name(source).unwrap_or_default();
        (name == READ && reads_into_array(&command, event, source))
            || MAPFILE_COMMANDS.contains(&name.as_str())
            || declares_array(&command, source)
    })
}

fn declares_array(command: &Command<'_>, source: &[u8]) -> bool {
    command.node.kind() == DECLARATION
        && command
            .literal_name(source)
            .is_some_and(|name| DECLARING_COMMANDS.contains(&name.as_str()))
        && ARRAY_FLAGS.iter().any(|flag| command.has_flag(flag, source))
}

fn reads_into_array(command: &Command<'_>, event: &Event, source: &[u8]) -> bool {
    gnu_options(READ_OPTIONS, command.arguments(), source).is_some_and(|options| {
        options
            .iter()
            .any(|(flag, option)| flag == READ_ARRAY_FLAG && option.value.start_byte() == event.position.byte)
    })
}

fn is_indexed(assignment: Node<'_>) -> bool {
    assignment
        .child_by_field_name(NAME_FIELD)
        .is_some_and(|target| target.kind() == SUBSCRIPT)
}

fn check_arrays(events: &[Event], nodes: &Nodes<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let source = context.source;
    let mut arrays: HashSet<&str> = ARRAY_VARIABLES.iter().copied().collect();
    for event in events {
        match event.kind {
            EventKind::Reference => {
                let is_plain = nodes
                    .expansions
                    .get(&event.position.byte)
                    .is_some_and(|expansion| braced_content(*expansion, source) == event.name);
                if is_plain && arrays.contains(event.name.as_str()) {
                    findings.report_at(ShellRule::Sc2128, event.position, &event.name);
                }
            }
            EventKind::Assignment | EventKind::Checked => {
                if is_array_write(event, nodes, context) {
                    arrays.insert(&event.name);
                } else if let Some(assignment) = nodes.assignments.get(&event.position.byte) {
                    report_string_into_array((*assignment, event), &arrays, source, findings);
                } else {
                    arrays.remove(event.name.as_str());
                }
            }
            EventKind::ScopeStart | EventKind::ScopeEnd => {}
        }
    }
}

fn report_string_into_array(
    (assignment, event): (Node<'_>, &Event),
    arrays: &HashSet<&str>,
    source: &[u8],
    findings: &mut Findings,
) {
    if !arrays.contains(event.name.as_str()) {
        return;
    }
    let rule = if is_appending(assignment, source) {
        ShellRule::Sc2179
    } else {
        ShellRule::Sc2178
    };
    findings.report_at(rule, event.position, &event.name);
}

fn is_appending(assignment: Node<'_>, source: &[u8]) -> bool {
    children(assignment)
        .into_iter()
        .any(|child| !child.is_named() && text(child, source) == APPEND)
}

// La expresión `"|([/= ]|^)'|'( |$)|\\ ` de ShellCheck.
fn contains_quotes(word: &str) -> bool {
    if word.contains(DOUBLE_QUOTE) || word.contains(ESCAPED_SPACE) {
        return true;
    }
    word.match_indices(SINGLE_QUOTE).any(|(index, _)| {
        let before = word.get(..index).unwrap_or_default();
        let after = word.get(index.saturating_add(1)..).unwrap_or_default();
        before.is_empty() || before.ends_with(BEFORE_QUOTE) || after.is_empty() || after.starts_with(' ')
    })
}

// `forToken`: dónde hay comillas literales en lo que se asigna, propias o de otra variable.
fn quote_position(piece: &Piece, quoted: &HashMap<String, Position>) -> Option<Position> {
    match piece.kind {
        PieceKind::Braced => quoted.get(&piece.text).copied(),
        PieceKind::DoubleQuoted => piece.inner.iter().find_map(|inner| quote_position(inner, quoted)),
        PieceKind::Literal
        | PieceKind::SingleQuoted
        | PieceKind::DollarSingleQuoted
        | PieceKind::Glob
        | PieceKind::Extglob
        | PieceKind::Brace
        | PieceKind::CommandSubstitution
        | PieceKind::Arithmetic
        | PieceKind::Other => {
            contains_quotes(&simplified_text(std::slice::from_ref(piece))).then_some(piece.position)
        }
    }
}

fn check_quotes(events: &[Event], nodes: &Nodes<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let source = context.source;
    let mut quoted: HashMap<String, Position> = HashMap::new();
    let mut reported = BTreeSet::new();
    for event in events {
        match event.kind {
            EventKind::Assignment => {
                let Some(value) = nodes
                    .assignments
                    .get(&event.position.byte)
                    .and_then(|assignment| assignment.child_by_field_name(VALUE_FIELD))
                    .filter(|value| value.kind() != ARRAY)
                else {
                    continue;
                };
                let found = pieces(value, source)
                    .iter()
                    .find_map(|piece| quote_position(piece, &quoted));
                match found {
                    Some(position) => quoted.insert(event.name.clone(), position),
                    None => quoted.remove(&event.name),
                };
            }
            EventKind::Reference => {
                let Some(origin) = quoted.get(&event.name) else {
                    continue;
                };
                if is_harmless_read(event, nodes, context) {
                    continue;
                }
                if reported.insert(origin.byte) {
                    findings.report_at(ShellRule::Sc2089, *origin, &event.name);
                }
                findings.report_at(ShellRule::Sc2090, event.position, &event.name);
            }
            EventKind::Checked | EventKind::ScopeStart | EventKind::ScopeEnd => {}
        }
    }
}

fn is_harmless_read(event: &Event, nodes: &Nodes<'_>, context: &Context<'_, '_>) -> bool {
    let source = context.source;
    nodes.expansions.get(&event.position.byte).map_or_else(
        || !is_declaration_argument(event, context),
        |expansion| {
            is_param_to_eval(*expansion, source)
                || is_quote_free(*expansion, context, Strictness::Strict)
                || braced_content(*expansion, source).starts_with(LENGTH_PREFIX)
        },
    )
}

// `export x` y `export x=` también leen la variable.
fn is_declaration_argument(event: &Event, context: &Context<'_, '_>) -> bool {
    context
        .root
        .descendant_for_byte_range(event.position.byte, event.position.byte)
        .is_some_and(|node| {
            std::iter::once(node)
                .chain(ancestors(node))
                .take_while(|ancestor| ancestor.start_byte() == event.position.byte)
                .any(|ancestor| {
                    ancestor
                        .parent()
                        .is_some_and(|parent| parent.kind() == DECLARATION)
                })
        })
}

fn is_param_to_eval(expansion: Node<'_>, source: &[u8]) -> bool {
    ancestors(expansion)
        .find_map(Command::of)
        .is_some_and(|command| command.is_command(EVAL, source))
}
