use tree_sitter::Node;

use crate::fix::{Fix, TextEdit};
use crate::lint::python::ast::{COMMENT, PARENTHESIZED, named, text};
use crate::lint::python::fix_edits::{has_comment_within, point_of};

const CONDITION_FIELD: &str = "condition";
const CONSEQUENCE_FIELD: &str = "consequence";
const BODY_FIELD: &str = "body";
const WITH_CLAUSE: &str = "with_clause";
const ASYNC_KEYWORD: &str = "async";
const ASYNC_WITH: &str = "async with";
const WITH_KEYWORD: &str = "with";
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
    let inner = only_statement(outer, CONSEQUENCE_FIELD)?;
    let keyword = if outer.kind() == ELIF {
        ELIF_KEYWORD
    } else {
        IF_KEYWORD
    };
    let header = format!(
        "{keyword} {} and {}",
        operand(outer.child_by_field_name(CONDITION_FIELD)?, source),
        operand(inner.child_by_field_name(CONDITION_FIELD)?, source)
    );
    lifted(outer, CONSEQUENCE_FIELD, &header, source).map(Fix::marked_unsafe)
}

// El arreglo de SIM117, como el `fix_multiple_with_statements` de ruff: `with a:` con un único
// `with b:` adentro pasa a `with a, b:` y el cuerpo de adentro sube un nivel. Sin arreglo si alguno
// usa la forma con paréntesis.
pub fn merge_with_fix(outer: Node<'_>, source: &[u8]) -> Option<Fix> {
    let inner = only_statement(outer, BODY_FIELD)?;
    let items = |statement: Node<'_>| {
        named(statement)
            .into_iter()
            .find(|child| child.kind() == WITH_CLAUSE)
            .map(|clause| text(clause, source))
            .filter(|clause| !clause.starts_with('('))
    };
    let prefix = if text(outer, source).starts_with(ASYNC_KEYWORD) {
        ASYNC_WITH
    } else {
        WITH_KEYWORD
    };
    let header = format!("{prefix} {}, {}", items(outer)?, items(inner)?);
    lifted(outer, BODY_FIELD, &header, source)
}

fn only_statement<'tree>(outer: Node<'tree>, body_field: &str) -> Option<Node<'tree>> {
    named(outer.child_by_field_name(body_field)?)
        .into_iter()
        .find(|child| child.kind() != COMMENT)
}

// La sentencia de afuera pasa a `header` con el cuerpo de la de adentro un nivel más arriba. Sin
// arreglo con comentarios entre las dos, si alguna no empieza su línea o si la primera línea no
// entra en 88 columnas.
fn lifted(outer: Node<'_>, body_field: &str, header: &str, source: &[u8]) -> Option<Fix> {
    let inner = only_statement(outer, body_field)?;
    let inner_body = inner.child_by_field_name(body_field)?;
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
    let inner_colon = (0..inner.child_count())
        .filter_map(|index| inner.child(index))
        .rev()
        .find(|child| child.kind() == COLON)
        .filter(|colon| colon.end_byte() <= inner_body.start_byte())?;
    let tail =
        String::from_utf8_lossy(source.get(inner_colon.start_byte()..inner_body.end_byte())?).into_owned();
    let outer_indent_text = String::from_utf8_lossy(outer_indent).into_owned();
    let header = format!("{outer_indent_text}{header}");
    let first_line = format!("{header}{}", tail.split('\n').next().unwrap_or_default());
    if first_line.chars().count() > MAX_LINE_LENGTH {
        return None;
    }
    let extra = String::from_utf8_lossy(inner_indent.get(outer_indent.len()..)?).into_owned();
    let nested_prefix = format!("{outer_indent_text}{extra}");
    let body: Vec<String> = tail
        .split_inclusive('\n')
        .enumerate()
        .map(|(index, line)| {
            if index == 0 || extra.is_empty() {
                line.to_owned()
            } else {
                line.strip_prefix(nested_prefix.as_str())
                    .map_or_else(|| line.to_owned(), |rest| format!("{outer_indent_text}{rest}"))
            }
        })
        .collect();
    Some(Fix::from_edits(
        source,
        vec![TextEdit {
            start: point_of(source, outer_start),
            end: point_of(source, inner_body.end_byte()),
            replacement: format!("{header}{}", body.concat()),
        }],
    ))
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
