use tree_sitter::Node;

use crate::fix::{Fix, TextEdit, TextPoint};
use crate::lint::python::ast::{
    BODY_FIELD, COMMENT, EXPRESSION_STATEMENT, FUNCTION, IMPORT, IMPORT_FROM, NAME_FIELD, STRING, named, text,
};
use crate::lint::python::codegen;
use crate::lint::python::fix_edits::point_of;

const NONE_TEXT: &str = "None";
const INDENT: &str = "    ";
const LINE_FEED: u8 = b'\n';
const PASS: &str = "pass_statement";
const ELLIPSIS: &str = "ellipsis";
const FUTURE_IMPORT: &str = "future_import_statement";

// El `move_initialization` de ruff para B006: el default pasa a `None` y, al principio del cuerpo
// (después del docstring y de los imports), `if x is None: x = <default>`. En un cuerpo que es sólo
// `...` o `pass`, nada más el `None`. Sin arreglo si el cuerpo empieza en la línea del `def`.
// Siempre inseguro.
pub fn mutable_default_fix(parameter: Node<'_>, default: Node<'_>, source: &[u8]) -> Option<Fix> {
    let function = parameter
        .parent()?
        .parent()
        .filter(|function| function.kind() == FUNCTION)?;
    let statements: Vec<Node<'_>> = named(function.child_by_field_name(BODY_FIELD)?)
        .into_iter()
        .filter(|statement| statement.kind() != COMMENT)
        .collect();
    let first = *statements.first()?;
    let mut edits = vec![TextEdit {
        start: TextPoint::from_point(default.start_position()),
        end: TextPoint::from_point(default.end_position()),
        replacement: NONE_TEXT.to_owned(),
    }];
    if statements.iter().all(|statement| is_placeholder(*statement)) {
        return Some(Fix::from_edits(source, edits).marked_unsafe());
    }
    let first_line_start = line_start(source, first.start_byte());
    let indentation = source.get(first_line_start..first.start_byte())?;
    if !indentation.iter().all(|byte| matches!(byte, b' ' | b'\t')) {
        return None;
    }
    let name = text(parameter.child_by_field_name(NAME_FIELD)?, source);
    let indentation = String::from_utf8_lossy(indentation);
    let mut content = format!(
        "{indentation}if {name} is None:\n{indentation}{INDENT}{name} = {}\n",
        codegen::unparse(default, 0, source)
    );
    let mut position = first_line_start;
    for (index, statement) in statements.iter().enumerate() {
        if !is_preamble(*statement) {
            break;
        }
        if let Some(next) = statements.get(index.saturating_add(1)) {
            position = line_start(source, next.start_byte());
        } else {
            let end = full_line_end(source, statement.end_byte());
            if end == source.len() && source.last() != Some(&LINE_FEED) {
                content.insert(0, char::from(LINE_FEED));
            }
            position = end;
            break;
        }
    }
    let at = point_of(source, position);
    edits.push(TextEdit {
        start: at,
        end: at,
        replacement: content,
    });
    Some(Fix::from_edits(source, edits).marked_unsafe())
}

fn is_placeholder(statement: Node<'_>) -> bool {
    statement.kind() == PASS
        || (statement.kind() == EXPRESSION_STATEMENT
            && named(statement)
                .first()
                .is_some_and(|value| matches!(value.kind(), ELLIPSIS | STRING)))
}

fn is_preamble(statement: Node<'_>) -> bool {
    matches!(statement.kind(), IMPORT | IMPORT_FROM | FUTURE_IMPORT)
        || (statement.kind() == EXPRESSION_STATEMENT
            && named(statement)
                .first()
                .is_some_and(|value| value.kind() == STRING))
}

fn line_start(source: &[u8], offset: usize) -> usize {
    source
        .get(..offset)
        .and_then(|before| before.iter().rposition(|byte| *byte == LINE_FEED))
        .map_or(0, |position| position.saturating_add(1))
}

fn full_line_end(source: &[u8], offset: usize) -> usize {
    source
        .get(offset..)
        .and_then(|after| after.iter().position(|byte| *byte == LINE_FEED))
        .map_or(source.len(), |position| {
            offset.saturating_add(position).saturating_add(1)
        })
}
