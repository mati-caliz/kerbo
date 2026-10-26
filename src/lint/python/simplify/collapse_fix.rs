use tree_sitter::Node;

use crate::fix::{Fix, TextEdit};
use crate::lint::python::ast::{COMMENT, PARENTHESIZED, named, text};
use crate::lint::python::fix_edits::{has_comment_within, point_of};

const CONDITION_FIELD: &str = "condition";
const CONSEQUENCE_FIELD: &str = "consequence";
const ELIF: &str = "elif_clause";
const ELIF_KEYWORD: &str = "elif";
const IF_KEYWORD: &str = "if";
const COLON: &str = ":";
const BOOLEAN_OPERATOR: &str = "boolean_operator";
const OPERATOR_FIELD: &str = "operator";
const OR: &str = "or";
const LOOSE: [&str; 3] = ["conditional_expression", "lambda", "named_expression"];
const MAX_LINE_LENGTH: usize = 88;
const LINE_FEED: u8 = b'\n';

// El arreglo de SIM102, como el `collapse_nested_if` de ruff con libcst: `if a:` con un único `if b:`
// adentro pasa a `if a and b:` (con paréntesis en un `or`, un `if`, una lambda o un `:=`) y el cuerpo
// de adentro sube un nivel. Sin arreglo con comentarios entre los dos `if`, si la sentencia no empieza
// su línea o si la primera línea no entra en 88 columnas. Siempre inseguro.
pub fn collapse_fix(outer: Node<'_>, source: &[u8]) -> Option<Fix> {
    let outer_body = outer.child_by_field_name(CONSEQUENCE_FIELD)?;
    let inner = named(outer_body)
        .into_iter()
        .find(|child| child.kind() != COMMENT)?;
    let inner_body = inner.child_by_field_name(CONSEQUENCE_FIELD)?;
    let first_inner = named(inner_body)
        .into_iter()
        .find(|child| child.kind() != COMMENT)?;
    if has_comment_within(inner, inner.start_byte(), first_inner.start_byte()) {
        return None;
    }
    let outer_start = line_start(source, outer.start_byte());
    let outer_indent = source.get(outer_start..outer.start_byte())?;
    let inner_start = line_start(source, inner.start_byte());
    let inner_indent = source.get(inner_start..inner.start_byte())?;
    let is_indentation = |bytes: &[u8]| bytes.iter().all(|byte| matches!(byte, b' ' | b'\t'));
    if !is_indentation(outer_indent)
        || !is_indentation(inner_indent)
        || !inner_indent.starts_with(outer_indent)
    {
        return None;
    }
    let keyword = if outer.kind() == ELIF {
        ELIF_KEYWORD
    } else {
        IF_KEYWORD
    };
    let combined = format!(
        "{keyword} {} and {}",
        operand(outer.child_by_field_name(CONDITION_FIELD)?, source),
        operand(inner.child_by_field_name(CONDITION_FIELD)?, source)
    );
    let inner_colon = (0..inner.child_count())
        .filter_map(|index| inner.child(index))
        .find(|child| child.kind() == COLON)?;
    let tail =
        String::from_utf8_lossy(source.get(inner_colon.start_byte()..inner_body.end_byte())?).into_owned();
    let outer_indent_text = String::from_utf8_lossy(outer_indent).into_owned();
    let header = format!("{outer_indent_text}{combined}");
    let first_line = format!("{header}{}", tail.split('\n').next().unwrap_or_default());
    if first_line.chars().count() > MAX_LINE_LENGTH {
        return None;
    }
    let extra = String::from_utf8_lossy(inner_indent.get(outer_indent.len()..)?).into_owned();
    let body: Vec<String> = tail
        .split_inclusive('\n')
        .enumerate()
        .map(|(index, line)| {
            if index == 0 || extra.is_empty() {
                line.to_owned()
            } else {
                line.strip_prefix(&format!("{outer_indent_text}{extra}"))
                    .map_or_else(|| line.to_owned(), |rest| format!("{outer_indent_text}{rest}"))
            }
        })
        .collect();
    let replacement = format!("{header}{}", body.concat());
    Some(
        Fix::from_edits(
            source,
            vec![TextEdit {
                start: point_of(source, outer_start),
                end: point_of(source, inner_body.end_byte()),
                replacement,
            }],
        )
        .marked_unsafe(),
    )
}

fn operand(test: Node<'_>, source: &[u8]) -> String {
    let written = text(test, source);
    let is_or = test.kind() == BOOLEAN_OPERATOR
        && test
            .child_by_field_name(OPERATOR_FIELD)
            .is_some_and(|operator| text(operator, source) == OR);
    if test.kind() != PARENTHESIZED && (is_or || LOOSE.contains(&test.kind())) {
        format!("({written})")
    } else {
        written.to_owned()
    }
}

fn line_start(source: &[u8], offset: usize) -> usize {
    source
        .get(..offset)
        .and_then(|before| before.iter().rposition(|byte| *byte == LINE_FEED))
        .map_or(0, |position| position.saturating_add(1))
}
