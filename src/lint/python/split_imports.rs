use tree_sitter::Node;

use super::ast::{ALIASED_IMPORT, NAME_FIELD, fields, text};
use crate::fix::Fix;

const ALIAS_FIELD: &str = "alias";
const SEMICOLON: u8 = b';';
const LINE_FEED: u8 = b'\n';
const CARRIAGE_RETURN: u8 = b'\r';
const CRLF: &str = "\r\n";
const LF: &str = "\n";
const STATEMENT_SEPARATOR: &str = "; ";

// E401, el `split_imports` de ruff: `import a, b` pasa a un import por línea con la misma sangría, o a
// `import a; import b` si la sentencia comparte la línea con otra. Detrás de algo que no es un `;`
// (`if x: import a, b`) ruff rompería el código y lo revierte: ahí no hay arreglo.
pub fn split_imports_fix(statement: Node<'_>, source: &[u8]) -> Option<Fix> {
    let imports: Vec<String> = fields(statement, NAME_FIELD)
        .into_iter()
        .map(|name| single_import(name, source))
        .collect();
    let line_start = source
        .get(..statement.start_byte())?
        .iter()
        .rposition(|byte| *byte == LINE_FEED)
        .map_or(0, |position| position.saturating_add(1));
    let before = source.get(line_start..statement.start_byte())?;
    let after = source.get(statement.end_byte()..)?;
    let after_on_line = after.split(|byte| *byte == LINE_FEED).next().unwrap_or_default();
    let preceded_by_semicolon = before.trim_ascii_end().last() == Some(&SEMICOLON);
    let followed_by_semicolon = after_on_line.trim_ascii_start().first() == Some(&SEMICOLON);
    if preceded_by_semicolon || followed_by_semicolon {
        return Some(Fix::replace(
            statement,
            &imports.join(STATEMENT_SEPARATOR),
            source,
        ));
    }
    if !before.iter().all(|byte| matches!(byte, b' ' | b'\t' | b'\x0c')) {
        return None;
    }
    let indentation = String::from_utf8_lossy(before);
    let line_ending = line_ending(source);
    let replacement = imports
        .iter()
        .map(|import| format!("{indentation}{import}"))
        .collect::<Vec<String>>()
        .join(line_ending);
    let start = tree_sitter::Point {
        row: statement.start_position().row,
        column: 0,
    };
    Some(Fix::replace_range(
        start,
        statement.end_position(),
        &replacement,
        source,
    ))
}

fn single_import(name: Node<'_>, source: &[u8]) -> String {
    if name.kind() != ALIASED_IMPORT {
        return format!("import {}", text(name, source));
    }
    let module = name
        .child_by_field_name(NAME_FIELD)
        .map_or("", |module| text(module, source));
    let alias = name
        .child_by_field_name(ALIAS_FIELD)
        .map_or("", |alias| text(alias, source));
    format!("import {module} as {alias}")
}

// El `Stylist` de ruff: el final de línea del primer salto del archivo.
fn line_ending(source: &[u8]) -> &'static str {
    let first_break = source.iter().position(|byte| *byte == LINE_FEED);
    let is_crlf = first_break
        .and_then(|position| position.checked_sub(1))
        .and_then(|before| source.get(before))
        == Some(&CARRIAGE_RETURN);
    if is_crlf { CRLF } else { LF }
}
