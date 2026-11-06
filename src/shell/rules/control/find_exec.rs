use tree_sitter::Node;

use crate::rules::ShellRule;
use crate::shell::commands::Command;
use crate::shell::context::Findings;
use crate::shell::syntax::{COMMAND, text};
use crate::shell::word_pieces::{PieceKind, literal_string, pieces};

const FIND: &str = "find";
const EXEC_FLAGS: &[&str] = &["-exec", "-execdir"];
const EXEC_TERMINATORS: &[&str] = &["+", ";"];

// SC2014 y SC2067: `checkFindExec`.
pub fn check(node: Node<'_>, source: &[u8], findings: &mut Findings) {
    if node.kind() != COMMAND {
        return;
    }
    let Some(command) = Command::of(node).filter(|command| command.is_command(FIND, source)) else {
        return;
    };
    let mut inside_exec = false;
    for word in command.arguments() {
        let word_pieces = pieces(*word, source);
        if inside_exec {
            for piece in word_pieces.iter().filter(|piece| {
                matches!(
                    piece.kind,
                    PieceKind::CommandSubstitution | PieceKind::Glob | PieceKind::Extglob
                )
            }) {
                findings.report_at(ShellRule::Sc2014, piece.position, text(*word, source));
            }
        }
        let literal = literal_string(&word_pieces).unwrap_or_default();
        if EXEC_FLAGS.contains(&literal.as_str()) {
            inside_exec = true;
        } else if EXEC_TERMINATORS.contains(&literal.as_str()) {
            inside_exec = false;
        }
    }
    if inside_exec && let Some(last) = command.words.last() {
        findings.report(ShellRule::Sc2067, *last, text(*last, source));
    }
}
