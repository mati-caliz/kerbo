use tree_sitter::Node;

use super::{EventKind, Flow};
use crate::shell::context::Position;
use crate::shell::parameters::reference;
use crate::shell::syntax::text;
use crate::shell::test_expression::{Condition, parse};
use crate::shell::word_pieces::{PieceKind, flattened, pieces};
use crate::shell::words::literal_or;

const CHECKING_OPERATORS: &[&str] = &["-n", "-z"];
const VARIABLE_IS_SET: &str = "-v";
const INDEX_START: char = '[';
const NOT_LITERAL: &str = " ";

// `markAsChecked`: lo que `[ -n "$x" ]`, `[ -z "$x" ]` y `[ "$x" ]` miran, y el nombre de `[ -v x ]`.
pub fn checked_variables(test: Node<'_>, source: &[u8], flow: &mut Flow) {
    let Some(condition) = parse(test, source) else {
        return;
    };
    let position = Position::of(test);
    condition.visit(true, &mut |part, _| match part {
        Condition::Nullary(word) => mark_checked(*word, source, position, flow),
        Condition::Unary(operator, word) => {
            let operator_text = text(*operator, source);
            if CHECKING_OPERATORS.contains(&operator_text) {
                mark_checked(*word, source, position, flow);
            } else if operator_text == VARIABLE_IS_SET {
                let name: String = literal_or(*word, source, NOT_LITERAL)
                    .chars()
                    .take_while(|character| *character != INDEX_START)
                    .collect();
                flow.push(EventKind::Checked, name, position);
            }
        }
        Condition::And(..)
        | Condition::Or(..)
        | Condition::Not(..)
        | Condition::Group(_)
        | Condition::Binary(..) => {}
    });
}

fn mark_checked(word: Node<'_>, source: &[u8], position: Position, flow: &mut Flow) {
    let word_pieces = pieces(word, source);
    for piece in flattened(&word_pieces) {
        if piece.kind == PieceKind::Braced {
            flow.push(EventKind::Checked, reference(&piece.text), position);
        }
    }
}
