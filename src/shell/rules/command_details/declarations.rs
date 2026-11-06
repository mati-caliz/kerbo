use std::collections::{BTreeSet, HashMap};

use tree_sitter::Node;

use crate::rules::ShellRule;
use crate::shell::commands::Command;
use crate::shell::context::Findings;
use crate::shell::parameters::reference;
use crate::shell::syntax::{
    ARITHMETIC_EXPANSION, ASSIGNMENT, COMMAND, DECLARATION, EXPANSIONS, VARIABLE_NAME, ancestors,
    descendants, text,
};
use crate::shell::variables::assigned_name;
use crate::shell::word_pieces::{Piece, PieceKind, pieces, unquoted_literal};
use crate::shell::words::braced_content;

const LET: &str = "let";
const ALIAS: &str = "alias";
const DECLARING_COMMANDS: &[&str] = &["local", "declare", "export", "readonly", "typeset", "let"];
const EQUALS: char = '=';
const APPEND: &str = "+=";

pub fn check(node: Node<'_>, source: &[u8], findings: &mut Findings) {
    if ![COMMAND, DECLARATION].contains(&node.kind()) {
        return;
    }
    let Some(command) = Command::of(node) else {
        return;
    };
    let Some(name) = command.literal_name(source) else {
        return;
    };
    if name == ALIAS || DECLARING_COMMANDS.contains(&name.as_str()) {
        check_spaced_equals(&command, &name, source, findings);
    }
    if DECLARING_COMMANDS.contains(&name.as_str()) {
        check_back_references(&command, source, findings);
        check_multiple_declaring(&command, source, findings);
    }
}

// SC2290: `checkArgComparison`.
fn check_spaced_equals(command: &Command<'_>, name: &str, source: &[u8], findings: &mut Findings) {
    for argument in command.arguments() {
        if argument.kind() == ASSIGNMENT {
            continue;
        }
        let word_pieces = pieces(*argument, source);
        let subject = text(*argument, source);
        if let Some(first) = word_pieces
            .first()
            .filter(|first| first.kind == PieceKind::Literal)
            && (first.text.starts_with(EQUALS) || first.text.starts_with(APPEND))
        {
            findings.report_at(ShellRule::Sc2290, first.position, subject);
        }
        if name == LET
            && let Some(last) = trailing_literal(&word_pieces)
            && last.text.ends_with(EQUALS)
        {
            findings.report_at(ShellRule::Sc2290, last.position, subject);
        }
    }
}

fn trailing_literal(word_pieces: &[Piece]) -> Option<&Piece> {
    word_pieces.last().filter(|last| last.kind == PieceKind::Literal)
}

// SC2316: `checkMultipleDeclaring`.
fn check_multiple_declaring(command: &Command<'_>, source: &[u8], findings: &mut Findings) {
    for argument in command.arguments() {
        let is_declaring = unquoted_literal(&pieces(*argument, source))
            .is_some_and(|literal| DECLARING_COMMANDS.contains(&literal.as_str()));
        if is_declaring {
            findings.report(ShellRule::Sc2316, *argument, text(*argument, source));
        }
    }
}

// SC2318: `checkBackreferencingDeclaration`.
fn check_back_references(command: &Command<'_>, source: &[u8], findings: &mut Findings) {
    let mut assigned: HashMap<String, Node<'_>> = HashMap::new();
    let mut reported = BTreeSet::new();
    for argument in command.arguments() {
        for name in references_in(*argument, source) {
            if let Some(definition) = assigned.get(&name)
                && reported.insert(definition.start_byte())
            {
                findings.report(ShellRule::Sc2318, *definition, &name);
            }
        }
        if argument.kind() == ASSIGNMENT
            && let Some(name) = assigned_name(*argument, source)
        {
            assigned.insert(name, *argument);
        }
    }
}

fn references_in(argument: Node<'_>, source: &[u8]) -> Vec<String> {
    descendants(argument)
        .into_iter()
        .filter_map(|node| {
            if EXPANSIONS.contains(&node.kind()) {
                return Some(reference(&braced_content(node, source)));
            }
            let in_arithmetic = node.kind() == VARIABLE_NAME
                && ancestors(node).any(|ancestor| ancestor.kind() == ARITHMETIC_EXPANSION)
                && !ancestors(node)
                    .next()
                    .is_some_and(|parent| EXPANSIONS.contains(&parent.kind()));
            in_arithmetic.then(|| text(node, source).to_owned())
        })
        .collect()
}
