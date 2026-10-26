use tree_sitter::{Node, Point};

use super::{ALIASED_IMPORT, MANDATORY_FUTURE_FEATURES, NAME_FIELD, NON_TEXT_PREFIXES};
use crate::fix::Fix;
use crate::lint::python::ast::{
    COMMENT, CONCATENATED_STRING, EXPRESSION_STATEMENT, STRING, named, string_prefix, text,
};
use crate::lint::python::fix_edits::{rewrite_names, safe_unless_commented};
use crate::lint::python::statement_removal::{delete_statement, delete_statement_only, isolation_group};

// UP009 borra la línea entera del comentario.
pub fn line_deletion(row: usize, source: &[u8]) -> Fix {
    let start = Point { row, column: 0 };
    let end = Point {
        row: row.saturating_add(1),
        column: 0,
    };
    Fix::replace_range(start, end, "", source)
}

// UP025: sacar la `u`; si antes hay la misma comilla (`""u""`), queda un espacio para no pegarlas.
pub fn unicode_prefix_fix(string: Node<'_>, source: &[u8]) -> Fix {
    let start = string.start_byte();
    let quote = source.get(start.saturating_add(1)).copied();
    let previous = start
        .checked_sub(1)
        .and_then(|before| source.get(before))
        .copied();
    let replacement = if previous.is_some() && previous == quote {
        " "
    } else {
        ""
    };
    let end = Point {
        row: string.start_position().row,
        column: string.start_position().column.saturating_add(1),
    };
    Fix::replace_range(string.start_position(), end, replacement, source)
}

// UP010 saca las características obligatorias; si no queda ninguna, borra la sentencia. Inseguro si
// el cambio pasa sobre un comentario.
pub fn future_import_fix(statement: Node<'_>, source: &[u8]) -> Fix {
    let mut cursor = statement.walk();
    let entries: Vec<Node<'_>> = statement
        .children_by_field_name(NAME_FIELD, &mut cursor)
        .collect();
    let kept: Vec<&str> = entries
        .iter()
        .filter(|entry| {
            let name = if entry.kind() == ALIASED_IMPORT {
                entry.child_by_field_name(NAME_FIELD).unwrap_or(**entry)
            } else {
                **entry
            };
            !MANDATORY_FUTURE_FEATURES.contains(&text(name, source))
        })
        .map(|entry| text(*entry, source))
        .collect();
    let rewritten = if kept.is_empty() {
        None
    } else {
        rewrite_names(statement, &entries, &kept.join(", "), source)
    };
    let fix = rewritten.map_or_else(
        || delete_statement(statement, source),
        |fix| fix.isolated(isolation_group(statement)),
    );
    safe_unless_commented(fix, statement)
}

// PIE790: si tiene un comentario al lado, hasta el comentario; si no, la sentencia entera sin dejar
// un `pass`. Inseguro si lo que sigue es un string, que pasaría a ser el docstring.
pub fn placeholder_fix(statement: Node<'_>, source: &[u8]) -> Fix {
    let comment = statement
        .next_sibling()
        .filter(|next| next.kind() == COMMENT && next.start_position().row == statement.end_position().row);
    let fix = comment.map_or_else(
        || delete_statement_only(statement, source),
        |comment| Fix::replace_range(statement.start_position(), comment.start_position(), "", source),
    );
    let next_is_string = std::iter::successors(statement.next_named_sibling(), Node::next_named_sibling)
        .find(|next| next.kind() != COMMENT)
        .is_some_and(|next| {
            next.kind() == EXPRESSION_STATEMENT
                && matches!(named(next).as_slice(), [only] if only.kind() == STRING || only.kind() == CONCATENATED_STRING)
                && !named(next).first().is_some_and(|only| string_prefix(*only, source).contains(NON_TEXT_PREFIXES))
        });
    let fix = fix.isolated(isolation_group(statement));
    if next_is_string { fix.marked_unsafe() } else { fix }
}
