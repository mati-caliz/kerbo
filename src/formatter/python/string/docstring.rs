use super::flags::StringFlags;
use super::indentation::Indentation;
use super::normalize::{NormalizedString, fmt_normalized};
use crate::formatter::document::LineMode;
use crate::formatter::python::context::Formatter;
use crate::formatter::python::trivia::is_python_whitespace;

const BACKSLASH: char = '\\';
const LINE_FEED: char = '\n';
const SPACE: char = ' ';

fn count_consecutive_chars_from_end(text: &str, target: char) -> usize {
    text.chars()
        .rev()
        .take_while(|character| *character == target)
        .count()
}

const fn is_odd(count: usize) -> bool {
    count % 2 == 1
}

// Si hace falta un espacio antes de las comillas que cierran: el contenido termina en una barra
// invertida sin escapar o en una comilla igual a las que cierran.
pub fn needs_chaperone_space(flags: StringFlags, trim_end: &str) -> bool {
    if is_odd(count_consecutive_chars_from_end(trim_end, BACKSLASH)) {
        return true;
    }
    if !flags.is_triple_quoted {
        return false;
    }
    trim_end
        .strip_suffix(flags.quote.as_char())
        .is_some_and(|before_quote| !is_odd(count_consecutive_chars_from_end(before_quote, BACKSLASH)))
}

// Una barra invertida al final de una línea une esa línea con la siguiente: el docstring no se
// puede reindentar sin cambiar su valor.
fn contains_unescaped_newline(haystack: &str) -> bool {
    let mut rest = haystack;
    while let Some(index) = rest.find(BACKSLASH) {
        rest = rest
            .get(index.saturating_add(BACKSLASH.len_utf8())..)
            .unwrap_or_default()
            .trim_start_matches(is_python_whitespace);
        if rest.starts_with(LINE_FEED) {
            return true;
        }
    }
    false
}

fn has_tab_or_non_ascii_space_indent(line: &str) -> bool {
    line.chars()
        .take_while(|character| character.is_whitespace())
        .any(|character| character != SPACE)
}

// Una línea del cuerpo sin la sangría común a todas; si su sangría tiene tabs, se reescribe con
// espacios.
fn print_line(f: &mut Formatter<'_>, line: &str, is_last: bool, stripped: Indentation) {
    let trim_end = line.trim_end();
    if trim_end.is_empty() {
        if !is_last {
            f.line(LineMode::Empty);
        }
        return;
    }
    if has_tab_or_non_ascii_space_indent(trim_end) {
        let indent_length = Indentation::of(trim_end)
            .columns()
            .saturating_sub(stripped.columns());
        let mut reindented = SPACE.to_string().repeat(indent_length);
        reindented.push_str(trim_end.trim_start());
        f.text(&reindented);
    } else {
        f.text(trim_end.get(stripped.text_len()..).unwrap_or_default());
    }
    if !is_last {
        f.line(LineMode::Hard);
    }
}

fn fmt_first_line(f: &mut Formatter<'_>, normalized: &NormalizedString<'_>, first: &str) -> bool {
    let docstring: &str = &normalized.text;
    let trim_end = first.trim_end();
    let trim_both = trim_end.trim_start();
    if trim_both.starts_with(normalized.flags.quote.as_char()) {
        f.space();
    }
    if !trim_end.is_empty() {
        f.text(trim_both);
    }
    let is_single_line = docstring.get(first.len()..).unwrap_or_default().trim().is_empty();
    if is_single_line
        && (needs_chaperone_space(normalized.flags, trim_end)
            || (trim_end.is_empty() && !docstring.is_empty()))
    {
        f.space();
    }
    is_single_line
}

// El docstring como lo deja ruff: la primera línea pegada a las comillas, el cuerpo sin su sangría
// original (queda con la del bloque) y sin espacios al final de las líneas.
pub fn fmt(f: &mut Formatter<'_>, normalized: &NormalizedString<'_>) {
    let docstring: &str = &normalized.text;
    if contains_unescaped_newline(docstring) {
        fmt_normalized(f, normalized);
        return;
    }
    let mut lines = docstring.split(LINE_FEED);
    f.token(normalized.flags.prefix());
    f.token(normalized.flags.quotes());
    let first = lines.next().unwrap_or_default();
    if fmt_first_line(f, normalized, first) {
        f.token(normalized.flags.quotes());
        return;
    }
    f.line(LineMode::Hard);
    let body: Vec<&str> = lines.collect();
    let stripped = body
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| Indentation::of(line))
        .min_by_key(|indentation| indentation.columns())
        .unwrap_or(Indentation::Spaces(0));
    let last = body.len().saturating_sub(1);
    for (index, line) in body.iter().enumerate() {
        print_line(f, line, index == last, stripped);
    }
    let trim_end =
        docstring.trim_end_matches(|character: char| character.is_whitespace() && character != LINE_FEED);
    if needs_chaperone_space(normalized.flags, trim_end) {
        f.space();
    }
    f.token(normalized.flags.quotes());
}
