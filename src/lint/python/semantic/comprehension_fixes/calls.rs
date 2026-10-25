use tree_sitter::Node;

use super::{
    BODY_FIELD, CLOSE_BRACE, CLOSE_PAREN, COMMA, FOR_IN_CLAUSE, GENERATOR, IF_CLAUSE, OPEN_BRACE, edit,
    elements, in_interpolation, single, unsafe_fix,
};
use crate::fix::Fix;
use crate::lint::python::ast::{COMMENT, named, text, unparenthesized};

// C419 con más de un argumento: los corchetes de la comprensión pasan a paréntesis.
pub fn comprehension_in_call_fix(comprehension: Node<'_>, source: &[u8]) -> Fix {
    let (start, end) = (comprehension.start_byte(), comprehension.end_byte());
    unsafe_fix(
        vec![
            edit(start, start.saturating_add(1), "(", source),
            edit(end.saturating_sub(1), end, ")", source),
        ],
        source,
    )
}

// C419 con un solo argumento: `any([x for x in y])` pasa a `any(x for x in y)`, sin los espacios
// pegados a los corchetes; una coma de más se va y quedan sus espacios. Con comentarios o saltos de
// línea adentro, libcst los reubica y acá no se arregla.
pub fn comprehension_argument_fix(call: Node<'_>, comprehension: Node<'_>, source: &[u8]) -> Option<Fix> {
    let raw = text(comprehension, source);
    if raw.contains('\n') || has_comment(comprehension) {
        return None;
    }
    let inner = raw.get(1..raw.len().saturating_sub(1))?.trim();
    let call_end = call.end_byte();
    let after = source
        .get(comprehension.end_byte()..call_end.saturating_sub(1))
        .unwrap_or_default();
    let comma = after.iter().position(|byte| *byte == COMMA);
    let edits = comma.map_or_else(
        || {
            vec![edit(
                comprehension.start_byte(),
                comprehension.end_byte(),
                inner,
                source,
            )]
        },
        |comma| {
            let whitespace =
                String::from_utf8_lossy(after.get(comma.saturating_add(1)..).unwrap_or_default());
            vec![edit(
                comprehension.start_byte(),
                call_end.saturating_sub(1),
                &format!("{inner}{whitespace}"),
                source,
            )]
        },
    );
    Some(unsafe_fix(edits, source))
}

fn has_comment(node: Node<'_>) -> bool {
    (0..node.child_count())
        .filter_map(|index| node.child(index))
        .any(|child| child.kind() == COMMENT || has_comment(child))
}

// C411: `list([x for x in y])` pasa a `[x for x in y]`.
pub fn list_call_fix(call: Node<'_>, argument: Node<'_>, source: &[u8]) -> Fix {
    unsafe_fix(
        vec![edit(
            call.start_byte(),
            call.end_byte(),
            text(argument, source),
            source,
        )],
        source,
    )
}

// C402 y C404: `dict((k, v) for ...)` y `dict([(k, v) for ...])` pasan a `{k: v for ...}`, como lo
// arma libcst: los espacios de adentro de los paréntesis de la llamada quedan adentro de las llaves.
pub fn dict_comprehension_fix(call: Node<'_>, argument: Node<'_>, source: &[u8]) -> Option<Fix> {
    let single = single(call, argument)?;
    let comprehension = single.node;
    let extra_parentheses =
        argument.id() != comprehension.id() && single.arguments.id() != comprehension.id();
    if single.parenthesized_generator || extra_parentheses {
        return None;
    }
    let element = comprehension.child_by_field_name(BODY_FIELD)?;
    let pair = elements(unparenthesized(element));
    let [key, value] = pair.as_slice() else {
        return None;
    };
    let clauses: Vec<Node<'_>> = named(comprehension)
        .into_iter()
        .filter(|child| matches!(child.kind(), FOR_IN_CLAUSE | IF_CLAUSE))
        .collect();
    let (first_clause, last_clause) = (clauses.first()?, clauses.last()?);
    let slice = |start: usize, end: usize| {
        String::from_utf8_lossy(source.get(start..end).unwrap_or_default()).into_owned()
    };
    let before_for = slice(element.end_byte(), first_clause.start_byte());
    let before_for = if before_for.is_empty() {
        String::from(" ")
    } else {
        before_for
    };
    let (content_start, content_end) = if comprehension.kind() == GENERATOR {
        (element.start_byte(), last_clause.end_byte())
    } else {
        (comprehension.start_byte(), comprehension.end_byte())
    };
    let open = slice(single.arguments.start_byte().saturating_add(1), content_start);
    let after_end = source
        .get(content_end..call.end_byte())
        .and_then(|rest| {
            rest.iter()
                .position(|byte| *byte == COMMA || *byte == CLOSE_PAREN)
        })
        .map_or(content_end, |offset| content_end.saturating_add(offset));
    let close = slice(content_end, after_end);
    let dictionary = format!(
        "{{{open}{}: {}{before_for}{}{close}}}",
        text(*key, source),
        text(*value, source),
        slice(first_clause.start_byte(), last_clause.end_byte()),
    );
    let before = call
        .start_byte()
        .checked_sub(1)
        .and_then(|index| source.get(index));
    let interpolated = in_interpolation(call);
    let left = if interpolated && before == Some(&OPEN_BRACE) {
        " "
    } else {
        ""
    };
    let right = if interpolated && source.get(call.end_byte()) == Some(&CLOSE_BRACE) {
        " "
    } else {
        ""
    };
    Some(unsafe_fix(
        vec![edit(
            call.start_byte(),
            call.end_byte(),
            &format!("{left}{dictionary}{right}"),
            source,
        )],
        source,
    ))
}
