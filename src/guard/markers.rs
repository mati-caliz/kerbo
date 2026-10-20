use tree_sitter::Node;

use crate::profile::{GuardShape, Profile};
use crate::syntax::one_based;

const NAME_FIELD: &str = "name";
const ANNOTATION_PREFIX: char = '@';
const ARGUMENTS_START: char = '(';
const PATH_SEPARATOR: char = '.';
const SHOWN_TEXT_LIMIT: usize = 60;
const CAST_TYPE_POSITION: u32 = 1;
const BODY_FIELD: &str = "body";
const CALLBACK_KINDS: &[&str] = &["arrow_function", "function_expression", "function"];
const BLOCK_KINDS: &[&str] = &["block", "statement_block", "statement_list"];
const PLACEHOLDER_STATEMENTS: &[&str] = &["pass_statement"];
const EXPRESSION_STATEMENT: &str = "expression_statement";
const PLACEHOLDER_EXPRESSIONS: &[&str] = &["string", "ellipsis"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkerKind {
    Suppression,
    DisabledTest,
    TypeEscape,
    TestDefinition,
    EmptyTest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marker {
    pub kind: MarkerKind,
    pub line: usize,
    pub end_line: usize,
    pub text: String,
}

pub fn find_markers(root: Node<'_>, source: &[u8], profile: &Profile) -> Vec<Marker> {
    let mut markers = Vec::new();
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        markers.extend(classify(node, source, profile));
        let mut cursor = node.walk();
        pending.extend(node.children(&mut cursor));
    }
    markers.sort_by_key(|marker| marker.line);
    markers
}

pub fn directive_marker(line: usize, text: &str) -> Marker {
    Marker {
        kind: MarkerKind::Suppression,
        line,
        end_line: line,
        text: shorten(text),
    }
}

fn classify(node: Node<'_>, source: &[u8], profile: &Profile) -> Vec<Marker> {
    let guard = &profile.guard;
    let kind = node.kind();
    let node_text = || node.utf8_text(source).unwrap_or_default();
    let marker = |marker_kind, shown: &str| Marker {
        kind: marker_kind,
        line: one_based(node.start_position().row),
        end_line: one_based(node.end_position().row),
        text: shorten(shown),
    };
    if profile.comments.contains(&kind) {
        return guard
            .suppression_comments
            .iter()
            .find(|&&needle| node_text().contains(needle))
            .map_or_else(Vec::new, |&needle| vec![marker(MarkerKind::Suppression, needle)]);
    }
    if guard.annotation_kinds.contains(&kind) {
        let found = annotation_markers(node, source, guard);
        let method = node.parent().and_then(|modifiers| modifiers.parent());
        let emptied = found.contains(&MarkerKind::TestDefinition)
            && method
                .and_then(|method| method.child_by_field_name(BODY_FIELD))
                .is_some_and(|body| is_empty_body(body, profile));
        let method_name = method
            .map(|method| function_name(method, source))
            .unwrap_or_default();
        return found
            .into_iter()
            .map(|marker_kind| marker(marker_kind, &annotation_name(node, source)))
            .chain(emptied.then(|| Marker {
                end_line: method.map_or(0, |method| one_based(method.end_position().row)),
                ..marker(MarkerKind::EmptyTest, &method_name)
            }))
            .collect();
    }
    if kind == guard.call_kind {
        let found = call_markers(node, source, guard);
        let emptied = found
            .iter()
            .any(|(marker_kind, _)| *marker_kind == MarkerKind::TestDefinition)
            && callback_body(node).is_some_and(|body| is_empty_body(body, profile));
        let shown = found.first().map(|(_, callee)| *callee).unwrap_or_default();
        return found
            .iter()
            .map(|&(marker_kind, callee)| marker(marker_kind, callee))
            .chain(emptied.then(|| marker(MarkerKind::EmptyTest, shown)))
            .collect();
    }
    if guard
        .test_function
        .is_some_and(|(function_kind, prefix)| kind == function_kind && defines_test(node, source, prefix))
    {
        let name = function_name(node, source);
        let emptied = node
            .child_by_field_name(BODY_FIELD)
            .is_some_and(|body| is_empty_body(body, profile));
        return std::iter::once(marker(MarkerKind::TestDefinition, &name))
            .chain(emptied.then(|| marker(MarkerKind::EmptyTest, &name)))
            .collect();
    }
    if is_type_escape(node, source, guard) {
        return vec![marker(MarkerKind::TypeEscape, node_text())];
    }
    Vec::new()
}

fn annotation_markers(node: Node<'_>, source: &[u8], guard: &GuardShape) -> Vec<MarkerKind> {
    let name = annotation_name(node, source);
    let named = |list: &[&str]| list.iter().any(|entry| annotation_matches(&name, entry));
    [
        (named(guard.suppression_annotations), MarkerKind::Suppression),
        (named(guard.disabled_annotations), MarkerKind::DisabledTest),
        (named(guard.test_annotations), MarkerKind::TestDefinition),
    ]
    .into_iter()
    .filter_map(|(found, kind)| found.then_some(kind))
    .collect()
}

fn annotation_name(node: Node<'_>, source: &[u8]) -> String {
    let named = node.child_by_field_name(NAME_FIELD).unwrap_or(node);
    let text = named
        .utf8_text(source)
        .unwrap_or_default()
        .trim()
        .trim_start_matches(ANNOTATION_PREFIX);
    text.split(ARGUMENTS_START)
        .next()
        .unwrap_or_default()
        .trim()
        .to_owned()
}

fn annotation_matches(name: &str, entry: &str) -> bool {
    name == entry || (!entry.contains(PATH_SEPARATOR) && name.rsplit(PATH_SEPARATOR).next() == Some(entry))
}

fn call_markers<'source>(
    node: Node<'_>,
    source: &'source [u8],
    guard: &GuardShape,
) -> Vec<(MarkerKind, &'source str)> {
    let Some(callee) = node
        .child_by_field_name("function")
        .and_then(|callee| callee.utf8_text(source).ok())
    else {
        return Vec::new();
    };
    [
        (guard.disabled_calls.contains(&callee), MarkerKind::DisabledTest),
        (guard.test_calls.contains(&callee), MarkerKind::TestDefinition),
    ]
    .into_iter()
    .filter_map(|(found, kind)| found.then_some((kind, callee)))
    .collect()
}

fn defines_test(node: Node<'_>, source: &[u8], prefix: &str) -> bool {
    function_name(node, source).starts_with(prefix)
}

fn function_name(node: Node<'_>, source: &[u8]) -> String {
    node.child_by_field_name(NAME_FIELD)
        .and_then(|name| name.utf8_text(source).ok())
        .unwrap_or_default()
        .to_owned()
}

fn is_type_escape(node: Node<'_>, source: &[u8], guard: &GuardShape) -> bool {
    let kind = node.kind();
    let text = node.utf8_text(source).unwrap_or_default();
    if guard
        .escape_types
        .iter()
        .any(|&(escape_kind, escape_text)| kind == escape_kind && text == escape_text)
    {
        return true;
    }
    guard.double_cast.is_some_and(|(cast_kind, middle_type)| {
        kind == cast_kind
            && node.named_child(0).is_some_and(|inner| {
                inner.kind() == cast_kind
                    && inner
                        .named_child(CAST_TYPE_POSITION)
                        .and_then(|cast| cast.utf8_text(source).ok())
                        == Some(middle_type)
            })
    })
}

fn callback_body(call: Node<'_>) -> Option<Node<'_>> {
    let arguments = call.child_by_field_name("arguments")?;
    let mut cursor = arguments.walk();
    let callback = arguments
        .named_children(&mut cursor)
        .filter(|argument| CALLBACK_KINDS.contains(&argument.kind()))
        .last()?;
    callback.child_by_field_name(BODY_FIELD)
}

fn is_empty_body(body: Node<'_>, profile: &Profile) -> bool {
    if !BLOCK_KINDS.contains(&body.kind()) {
        return false;
    }
    let mut cursor = body.walk();
    body.named_children(&mut cursor).all(|statement| {
        profile.comments.contains(&statement.kind())
            || PLACEHOLDER_STATEMENTS.contains(&statement.kind())
            || (BLOCK_KINDS.contains(&statement.kind()) && is_empty_body(statement, profile))
            || (statement.kind() == EXPRESSION_STATEMENT
                && statement
                    .named_child(0)
                    .is_some_and(|expression| PLACEHOLDER_EXPRESSIONS.contains(&expression.kind())))
    })
}

fn shorten(text: &str) -> String {
    let single_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    single_line.chars().take(SHOWN_TEXT_LIMIT).collect()
}
