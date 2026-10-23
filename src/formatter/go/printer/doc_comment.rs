use super::super::ast::Comment;

const LINE_PREFIX: &str = "//";
const DIRECTIVE_PREFIXES: &[&str] = &["line ", "extern ", "export "];
const DIRECTIVE_SEPARATOR: char = ':';

// `formatDocComment` de go/printer, con el texto de cada línea ya canónico. Sólo toca un grupo de
// comentarios `//`: uno con `/* */` queda como está. Las directivas (`//go:noinline`, `//export f`)
// van al final, separadas del texto por un `//`, y el texto pierde las líneas en blanco de los
// bordes; un grupo de sólo directivas no se toca. Todas las
// líneas quedan en la posición de la primera.
pub fn format_doc_comment(list: &[Comment]) -> Vec<Comment> {
    let Some(first) = list.first() else {
        return Vec::new();
    };
    if !list.iter().all(Comment::is_line_comment) {
        return list.to_vec();
    }
    let (directives, mut text): (Vec<&Comment>, Vec<&Comment>) = list
        .iter()
        .partition(|comment| is_directive(comment.text.trim_start_matches(LINE_PREFIX)));
    while text.last().is_some_and(|comment| is_blank(comment)) {
        text.pop();
    }
    let leading_blanks = text.iter().take_while(|comment| is_blank(comment)).count();
    text.drain(..leading_blanks);
    if text.is_empty() {
        return list.to_vec();
    }
    let at_first = |line: &str| Comment {
        slash: first.slash,
        text: line.to_owned(),
    };
    let mut out: Vec<Comment> = text.iter().map(|comment| at_first(&comment.text)).collect();
    if !directives.is_empty() {
        out.push(at_first(LINE_PREFIX));
        out.extend(directives.iter().map(|comment| at_first(&comment.text)));
    }
    out
}

fn is_blank(comment: &Comment) -> bool {
    comment.text.trim_start_matches(LINE_PREFIX).trim().is_empty()
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
