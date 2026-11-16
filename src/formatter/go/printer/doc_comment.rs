mod blocks;
mod spans;
mod text;

use super::super::ast::Comment;
use blocks::reformat;

const LINE_PREFIX: &str = "//";
const BLOCK_OPEN: &str = "/*";
const BLOCK_CLOSE: &str = "*/";
const BLOCK_LINE_STAR: char = '*';
const BLOCK_LINE_INDENT: [char; 2] = [' ', '\t'];
const CODE_INDENT: char = '\t';
const DIRECTIVE_PREFIXES: &[&str] = &["line ", "extern ", "export "];
const DIRECTIVE_SEPARATOR: char = ':';

// `formatDocComment` de go/printer: el texto pasa por el port de go/doc/comment y vuelve como un
// solo `/* */` o como líneas `//`, todas en la posición de la primera. Las directivas
// (`//go:noinline`, `//export f`) van al final, separadas del texto por un `//`; un grupo de sólo
// directivas, uno que mezcla `//` con `/* */` y un `/* */` de una línea o con `*` al principio de
// cada línea quedan como están. Si no queda texto, el comentario desaparece.
pub fn format_doc_comment(list: &[Comment]) -> Vec<Comment> {
    let Some(first) = list.first() else {
        return Vec::new();
    };
    let at_first = |text: String| Comment {
        slash: first.slash,
        text,
    };
    if !first.is_line_comment() {
        let formatted =
            block_comment_text(list).map(|body| format!("{BLOCK_OPEN}\n{}{BLOCK_CLOSE}", reformat(body)));
        return formatted.map_or_else(|| list.to_vec(), |text| vec![at_first(text)]);
    }
    let Some(after_slashes) = list
        .iter()
        .map(|comment| comment.text.strip_prefix(LINE_PREFIX))
        .collect::<Option<Vec<&str>>>()
    else {
        return list.to_vec();
    };
    let (directives, text_lines): (Vec<&str>, Vec<&str>) =
        after_slashes.into_iter().partition(|after| is_directive(after));
    if text_lines.is_empty() {
        return list.to_vec();
    }
    let text: String = text_lines
        .iter()
        .flat_map(|after| [after.strip_prefix(' ').unwrap_or(after), "\n"])
        .collect();
    let mut out: Vec<Comment> = reformat(&text)
        .lines()
        .map(|line| at_first(line_comment(line)))
        .collect();
    if !directives.is_empty() {
        out.push(at_first(LINE_PREFIX.to_owned()));
        out.extend(
            directives
                .iter()
                .map(|directive| at_first(format!("{LINE_PREFIX}{directive}"))),
        );
    }
    out
}

fn line_comment(line: &str) -> String {
    if line.is_empty() {
        LINE_PREFIX.to_owned()
    } else if line.starts_with(CODE_INDENT) {
        format!("{LINE_PREFIX}{line}")
    } else {
        format!("{LINE_PREFIX} {line}")
    }
}

fn block_comment_text(list: &[Comment]) -> Option<&str> {
    let [only] = list else {
        return None;
    };
    let has_stars_on_every_line = only.text.split('\n').skip(1).all(|line| {
        line.trim_start_matches(BLOCK_LINE_INDENT)
            .starts_with(BLOCK_LINE_STAR)
    });
    if has_stars_on_every_line {
        return None;
    }
    only.text.strip_prefix(BLOCK_OPEN)?.strip_suffix(BLOCK_CLOSE)
}

// `isDirective` de go/ast: `line `, `extern `, `export ` o `[a-z0-9]+:[a-z0-9]`.
fn is_directive(after_slashes: &str) -> bool {
    if DIRECTIVE_PREFIXES
        .iter()
        .any(|prefix| after_slashes.starts_with(prefix))
    {
        return true;
    }
    let Some(colon) = after_slashes.find(DIRECTIVE_SEPARATOR) else {
        return false;
    };
    let lower_or_digit = |byte: &u8| byte.is_ascii_lowercase() || byte.is_ascii_digit();
    let bytes = after_slashes.as_bytes();
    colon > 0
        && bytes
            .get(..colon)
            .is_some_and(|name| name.iter().all(lower_or_digit))
        && bytes.get(colon + 1).is_some_and(lower_or_digit)
}
