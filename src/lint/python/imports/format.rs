use super::model::{Alias, AliasComments};
use super::normalize::{FromStatement, Straight};

const LINE_LENGTH: usize = 88;
const TAB_SIZE: usize = 4;
const INDENTATION: &str = "    ";
const NEWLINE: char = '\n';
const COMMENT_GAP: &str = "  ";
const STAR: &str = "*";
const HASH: &str = "#";
const NOQA: &str = "noqa";
const NOSEC: &str = "nosec";
const COLON_PRAGMAS: [&str; 8] = [
    "isort", "type", "pyright", "pyrefly", "pylint", "flake8", "ruff", "ty",
];

// `format_import`: `import x` o `import x as y`, con sus comentarios.
pub fn format_straight(import: &Straight<'_>, is_first: bool) -> String {
    let mut output = atop_lines(&import.atop, is_first);
    output.push_str("import ");
    output.push_str(&alias_text(import.alias));
    for comment in &import.inline {
        output.push_str(COMMENT_GAP);
        output.push_str(comment);
    }
    output.push(NEWLINE);
    output
}

// `format_import_from`: en una línea si entra y ningún nombre tiene comentarios; si no, con
// paréntesis y un nombre por línea.
pub fn format_from(statement: &FromStatement<'_>, is_first: bool, indentation: &str) -> String {
    let only_star =
        matches!(statement.aliases.as_slice(), [(alias, _)] if alias.name == STAR && alias.asname.is_none());
    if only_star {
        return single_line(statement, is_first).0;
    }
    let inlinable = !statement.trailing_comma
        && (statement.aliases.len() == 1
            || statement.aliases.iter().all(|(_, comments)| comments.is_empty()));
    if inlinable {
        let (line, width) = single_line(statement, is_first);
        let fits = width.saturating_add(display_width(indentation, 0)) <= LINE_LENGTH;
        if fits || statement.aliases.iter().any(|(alias, _)| alias.name == STAR) {
            return line;
        }
    }
    multi_line(statement, is_first)
}

fn atop_lines(atop: &[&str], is_first: bool) -> String {
    let mut output = String::new();
    if !is_first && !atop.is_empty() {
        output.push(NEWLINE);
    }
    for comment in atop {
        output.push_str(comment);
        output.push(NEWLINE);
    }
    output
}

fn alias_text(alias: Alias<'_>) -> String {
    alias.asname.map_or_else(
        || alias.name.to_owned(),
        |asname| format!("{} as {asname}", alias.name),
    )
}

// La línea y su ancho, sin contar la sangría.
fn single_line(statement: &FromStatement<'_>, is_first: bool) -> (String, usize) {
    let mut output = atop_lines(&statement.comments.atop, is_first);
    let names: Vec<String> = statement
        .aliases
        .iter()
        .map(|(alias, _)| alias_text(*alias))
        .collect();
    let mut line = format!("from {} import {}", statement.module.name(), names.join(", "));
    let mut width = display_width(&line, 0);
    let alias_comments = statement
        .aliases
        .iter()
        .flat_map(|(_, comments)| all_comments(comments));
    for comment in statement
        .comments
        .inline
        .iter()
        .chain(alias_comments)
        .chain(&statement.comments.trailing)
    {
        line.push_str(COMMENT_GAP);
        line.push_str(comment);
        width = comment_width(comment, width);
    }
    output.push_str(&line);
    output.push(NEWLINE);
    (output, width)
}

fn all_comments<'list, 'source>(
    comments: &'list AliasComments<'source>,
) -> impl Iterator<Item = &'list &'source str> {
    comments
        .atop
        .iter()
        .chain(&comments.inline)
        .chain(&comments.trailing)
}

fn push_indented_line(output: &mut String, text: &str) {
    output.push_str(INDENTATION);
    output.push_str(text);
    output.push(NEWLINE);
}

fn multi_line(statement: &FromStatement<'_>, is_first: bool) -> String {
    let mut output = atop_lines(&statement.comments.atop, is_first);
    output.push_str("from ");
    output.push_str(&statement.module.name());
    output.push_str(" import (");
    for comment in &statement.comments.inline {
        output.push_str(COMMENT_GAP);
        output.push_str(comment);
    }
    output.push(NEWLINE);
    for (alias, comments) in &statement.aliases {
        for comment in &comments.atop {
            push_indented_line(&mut output, comment);
        }
        output.push_str(INDENTATION);
        output.push_str(&alias_text(*alias));
        output.push(',');
        for comment in &comments.inline {
            output.push_str(COMMENT_GAP);
            output.push_str(comment);
        }
        output.push(NEWLINE);
        for comment in &comments.trailing {
            push_indented_line(&mut output, comment);
        }
    }
    output.push(')');
    for comment in &statement.comments.trailing {
        output.push_str(COMMENT_GAP);
        output.push_str(comment);
    }
    output.push(NEWLINE);
    output
}

// `LineWidthBuilder`: cada carácter cuenta uno y el tab llega a la siguiente columna múltiplo de 4.
fn display_width(text: &str, start: usize) -> usize {
    text.chars().fold(start, |width, character| {
        if character == '\t' {
            width.saturating_add(TAB_SIZE.saturating_sub(width % TAB_SIZE))
        } else {
            width.saturating_add(1)
        }
    })
}

// `add_comment`: el comentario cuenta con sus dos espacios, salvo desde el primer pragma (`# noqa`,
// `# type:` y compañía), que no tiene que hacer partir la línea.
fn comment_width(comment: &str, width: usize) -> usize {
    let counted = pragma_offset(comment).map_or(comment, |offset| {
        comment.get(..offset).unwrap_or_default().trim_end()
    });
    if counted.is_empty() {
        width
    } else {
        display_width(counted, width.saturating_add(COMMENT_GAP.len()))
    }
}

fn pragma_offset(comment: &str) -> Option<usize> {
    comment
        .match_indices(HASH)
        .map(|(offset, _)| offset)
        .find(|offset| comment.get(*offset..).is_some_and(is_pragma))
}

// `is_pragma_comment` de ruff.
fn is_pragma(comment: &str) -> bool {
    let Some(content) = comment.strip_prefix(HASH) else {
        return false;
    };
    let content = content.trim_start();
    let is_noqa = content
        .get(..NOQA.len())
        .is_some_and(|start| start.eq_ignore_ascii_case(NOQA));
    is_noqa
        || content.starts_with(NOSEC)
        || content
            .split_once(':')
            .is_some_and(|(pragma, _)| COLON_PRAGMAS.contains(&pragma))
}
