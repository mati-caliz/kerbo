use tree_sitter::Node;

use crate::fix::{Fix, TextEdit};
use crate::lint::python::ast::{COMMENT, named};
use crate::lint::python::fix_edits::{delete_between, point_of};
use crate::lint::python::statement_removal::delete_statement;

const ELIF: &str = "elif_clause";
const ELSE_KEYWORD: &str = "else";
const CONDITION_FIELD: &str = "condition";
const CONSEQUENCE_FIELD: &str = "consequence";
const ALTERNATIVE_FIELD: &str = "alternative";
const BODY_FIELD: &str = "body";
const STRING: &str = "string";
const ELIF_PREFIX_LENGTH: usize = "el".len();
const LINE_FEED: u8 = b'\n';
const CARRIAGE_RETURN: u8 = b'\r';
const FORM_FEED: char = '\x0c';
const HASH: char = '#';
const TAB: char = '\t';
const SPACE: char = ' ';

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    AlwaysTrue,
    AlwaysFalse,
}

// Los `fix_always_true_branch` y `fix_always_false_branch` de ruff. `branch` es 0 para el `if` y n
// para el n-ésimo `elif`.
pub fn outdated_block_fix(
    source: &[u8],
    statement: Node<'_>,
    branch: usize,
    verdict: Verdict,
) -> Option<Fix> {
    let clauses: Vec<Node<'_>> = {
        let mut cursor = statement.walk();
        statement
            .children_by_field_name(ALTERNATIVE_FIELD, &mut cursor)
            .collect()
    };
    let statement_end = clauses
        .last()
        .map_or_else(|| body_of(statement), |clause| body_of(*clause))
        .and_then(|body| statements(body).last().map(Node::end_byte))?;
    let fix = match (verdict, branch.checked_sub(1)) {
        (Verdict::AlwaysFalse, None) => match clauses.first() {
            None => delete_statement(statement, source).isolated(None),
            Some(elif) if elif.kind() == ELIF => delete_between(
                statement.start_byte(),
                elif.start_byte().saturating_add(ELIF_PREFIX_LENGTH),
                source,
            ),
            Some(else_clause) => promoted_body(source, statement, body_of(*else_clause)?, statement_end)?,
        },
        (Verdict::AlwaysFalse, Some(elif_index)) => {
            let elif = elif_clauses(&clauses).get(elif_index).copied()?;
            let next_start = clauses
                .iter()
                .map(Node::start_byte)
                .find(|start| *start > elif.start_byte());
            let elif_end = statements(body_of(elif)?).last()?.end_byte();
            delete_between(elif.start_byte(), next_start.unwrap_or(elif_end), source)
        }
        (Verdict::AlwaysTrue, None) => promoted_body(source, statement, body_of(statement)?, statement_end)?,
        (Verdict::AlwaysTrue, Some(elif_index)) => {
            let elif = elif_clauses(&clauses).get(elif_index).copied()?;
            let test_end = elif.child_by_field_name(CONDITION_FIELD)?.end_byte();
            let body_end = statements(body_of(elif)?).last()?.end_byte();
            let kept = String::from_utf8_lossy(source.get(test_end..body_end)?);
            replace_bytes(
                source,
                elif.start_byte(),
                statement_end,
                &format!("{ELSE_KEYWORD}{kept}"),
            )
        }
    };
    Some(fix.marked_unsafe())
}

fn elif_clauses<'tree>(clauses: &[Node<'tree>]) -> Vec<Node<'tree>> {
    clauses
        .iter()
        .copied()
        .filter(|clause| clause.kind() == ELIF)
        .collect()
}

fn body_of(clause: Node<'_>) -> Option<Node<'_>> {
    clause
        .child_by_field_name(CONSEQUENCE_FIELD)
        .or_else(|| clause.child_by_field_name(BODY_FIELD))
}

fn statements(body: Node<'_>) -> Vec<Node<'_>> {
    named(body)
        .into_iter()
        .filter(|child| child.kind() != COMMENT)
        .collect()
}

// El cuerpo que queda en lugar del `if` entero: tal cual si está en la misma línea, o con la sangría
// del `if` (el `adjust_indentation` de ruff, sin el camino de libcst).
fn promoted_body(source: &[u8], statement: Node<'_>, body: Node<'_>, statement_end: usize) -> Option<Fix> {
    let body_statements = statements(body);
    let first = body_statements.first()?;
    let last = body_statements.last()?;
    if indentation(source, first.start_byte()).is_none() {
        let kept = String::from_utf8_lossy(source.get(first.start_byte()..last.end_byte())?);
        return Some(replace_bytes(
            source,
            statement.start_byte(),
            statement_end,
            &kept,
        ));
    }
    let target = indentation(source, statement.start_byte())?;
    let contents_start = line_start(source, first.start_byte());
    let contents = String::from_utf8_lossy(source.get(contents_start..last.end_byte())?).into_owned();
    if has_multiline_string(body) || has_mixed_indentation(&contents) {
        return None;
    }
    let dedented = dedent_to(&contents, &target)?;
    Some(replace_bytes(
        source,
        line_start(source, statement.start_byte()),
        statement_end,
        &dedented,
    ))
}

fn replace_bytes(source: &[u8], start: usize, end: usize, replacement: &str) -> Fix {
    Fix::from_edits(
        source,
        vec![TextEdit {
            start: point_of(source, start),
            end: point_of(source, end),
            replacement: replacement.to_owned(),
        }],
    )
}

fn line_start(source: &[u8], offset: usize) -> usize {
    source
        .get(..offset)
        .and_then(|before| before.iter().rposition(|byte| *byte == LINE_FEED))
        .map_or(0, |position| position.saturating_add(1))
}

// El `indentation` de ruff: lo que hay antes en la línea, si es todo espacio.
fn indentation(source: &[u8], offset: usize) -> Option<String> {
    let before = source.get(line_start(source, offset)..offset)?;
    let before = String::from_utf8_lossy(before);
    before
        .chars()
        .all(|character| matches!(character, SPACE | TAB | FORM_FEED))
        .then(|| before.into_owned())
}

fn has_multiline_string(node: Node<'_>) -> bool {
    let mut pending = vec![node];
    while let Some(current) = pending.pop() {
        if current.kind() == STRING && current.start_position().row != current.end_position().row {
            return true;
        }
        pending.extend(named(current));
    }
    false
}

fn has_mixed_indentation(contents: &str) -> bool {
    contents.lines().any(|line| {
        let trimmed = line.trim_start();
        let leading = line
            .get(..line.len().saturating_sub(trimmed.len()))
            .unwrap_or_default();
        !trimmed.is_empty() && leading.contains(TAB) && leading.contains(SPACE)
    })
}

// El `dedent_to` de ruff: la primera línea con código (o, si no hay, el primer comentario) marca la
// sangría actual; cada línea pierde la diferencia y las vacías quedan vacías.
fn dedent_to(contents: &str, indent: &str) -> Option<String> {
    let indent = indent.trim_start_matches(FORM_FEED);
    let mut first_comment_indent = None;
    let existing = contents
        .split_inclusive(char::from(LINE_FEED))
        .find_map(|line| {
            let without_feeds = line.trim_start_matches(FORM_FEED);
            let trimmed = without_feeds.trim_start();
            if trimmed.is_empty() {
                return None;
            }
            let length = without_feeds.len().saturating_sub(trimmed.len());
            if trimmed.starts_with(HASH) {
                first_comment_indent.get_or_insert(length);
                None
            } else {
                Some(length)
            }
        })
        .unwrap_or_else(|| first_comment_indent.unwrap_or_default());
    let removed = existing.checked_sub(indent.len())?;
    let mut result = String::with_capacity(contents.len());
    for line in contents.split_inclusive(char::from(LINE_FEED)) {
        let (body, ending) = split_line_ending(line);
        let content = body.trim_start_matches(FORM_FEED);
        let feeds = body
            .get(..body.len().saturating_sub(content.len()))
            .unwrap_or_default();
        let without_indent = body.trim_start();
        if without_indent.is_empty() {
            result.push_str(ending);
            continue;
        }
        let current = content.len().saturating_sub(without_indent.len());
        if current < existing {
            result.push_str(line);
            continue;
        }
        result.push_str(feeds);
        result.push_str(content.get(removed..).unwrap_or_default());
        result.push_str(ending);
    }
    Some(result)
}

fn split_line_ending(line: &str) -> (&str, &str) {
    let without_feed = line.strip_suffix(char::from(LINE_FEED));
    let Some(without_feed) = without_feed else {
        return (line, "");
    };
    let body = without_feed
        .strip_suffix(char::from(CARRIAGE_RETURN))
        .unwrap_or(without_feed);
    (body, line.get(body.len()..).unwrap_or_default())
}
