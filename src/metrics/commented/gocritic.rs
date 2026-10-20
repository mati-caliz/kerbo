use std::sync::LazyLock;

use regex::Regex;
use tree_sitter::Node;

use super::{CommentedCode, SnippetLanguage, parse_snippet};
use crate::syntax::{descendants, named_children};

const COMMENT: &str = "comment";
const LINE_TERMINATOR: &str = "\n";
const FUNCTION_DECLARATION: &str = "function_declaration";
const FUNCTIONS: &[&str] = &[FUNCTION_DECLARATION, "method_declaration"];
const BODY: &str = "body";
const SKIP_MARKERS: &[&str] = &["TODO", "http://", "https://", "e.g. "];
const OUTPUT_MARKERS: &[&str] = &["print", "fmt.", "log."];
const MIN_LENGTH: usize = 15;
const EXAMPLE_PREFIX: &str = "Example";
const EXAMPLE_OUTPUT: &str = "Output:";
const DIRECTIVE_PREFIXES: &[&str] = &["line ", "extern ", "export "];
const GO_WHITESPACE: &[char] = &[' ', '\t', '\n', '\r'];
const WRAPPER_PREFIX: &str = "package main;func main() {";
const WRAPPER_SUFFIX: &str = "}";
const CALLS: &[&str] = &["call_expression", "type_conversion_expression"];
const RECEIVE: &str = "<-";
const BLOCK_START: &str = "/*";

static NOT_QUITE_FUNC_CALL: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"[0-9A-Za-z_]+[\t\n\f\r ]+\([^)]*\)[\t\n\f\r ]*\z").ok());

#[derive(Debug, Clone, Copy)]
struct Comment<'tree> {
    node: Node<'tree>,
    start_row: usize,
    end_row: usize,
    trailing: bool,
    after_code: bool,
}

struct Function {
    start: usize,
    end: usize,
    is_example: bool,
}

// Calca commentedOutCode de gocritic (v0.15): los grupos de comentarios que arma go/parser, sólo
// dentro de funciones con cuerpo, el texto de CommentGroup.Text() y la prueba de que ese texto
// compila como una sentencia que no es una expresión suelta.
pub fn reported(root: Node<'_>, source: &[u8]) -> Vec<CommentedCode> {
    let functions = functions(root, source);
    let mut found = Vec::new();
    for group in comment_groups(&comments(root)) {
        let Some(first) = group.first() else {
            continue;
        };
        let start = first.node.start_byte();
        let Some(function) = functions
            .iter()
            .find(|function| function.start <= start && start <= function.end)
        else {
            continue;
        };
        for visited in split_block_comments(&group, source) {
            if looks_like_code(&visited, function, source) {
                found.extend(visited.first().map(|comment| super::at(comment.node, source)));
            }
        }
    }
    found
}

fn looks_like_code(group: &[Comment<'_>], function: &Function, source: &[u8]) -> bool {
    let text = group_text(group, source);
    if SKIP_MARKERS.iter().any(|marker| text.contains(marker)) {
        return false;
    }
    if text.chars().count() < MIN_LENGTH && !OUTPUT_MARKERS.iter().any(|marker| text.contains(marker)) {
        return false;
    }
    if NOT_QUITE_FUNC_CALL
        .as_ref()
        .is_some_and(|pattern| pattern.is_match(&text))
    {
        return false;
    }
    if function.is_example && text.contains(EXAMPLE_OUTPUT) {
        return false;
    }
    match single_statement(&text) {
        Statement::Permitted => false,
        Statement::Code => true,
        Statement::Invalid => is_code_block(&text),
    }
}

fn functions(root: Node<'_>, source: &[u8]) -> Vec<Function> {
    descendants(root)
        .into_iter()
        .filter(|node| FUNCTIONS.contains(&node.kind()) && node.child_by_field_name(BODY).is_some())
        .map(|node| {
            let name = node
                .child_by_field_name("name")
                .and_then(|name| name.utf8_text(source).ok())
                .unwrap_or_default();
            let has_parameters = node.child_by_field_name("parameters").is_some_and(|parameters| {
                !named_children(parameters)
                    .iter()
                    .all(|child| child.kind() == COMMENT)
            });
            Function {
                start: node.start_byte(),
                end: node.end_byte(),
                is_example: name.starts_with(EXAMPLE_PREFIX) && !has_parameters,
            }
        })
        .collect()
}

fn comments(root: Node<'_>) -> Vec<Comment<'_>> {
    let mut found = Vec::new();
    let mut last_code_row: Option<usize> = None;
    let mut code_since_comment = true;
    for leaf in descendants(root)
        .into_iter()
        .filter(|node| node.child_count() == 0 || node.kind() == COMMENT)
    {
        if leaf.kind() == COMMENT {
            let start_row = leaf.start_position().row;
            found.push(Comment {
                node: leaf,
                start_row,
                end_row: leaf.end_position().row,
                trailing: last_code_row == Some(start_row),
                after_code: code_since_comment,
            });
            code_since_comment = false;
            continue;
        }
        if leaf.kind() == LINE_TERMINATOR || leaf.start_byte() == leaf.end_byte() {
            continue;
        }
        last_code_row = Some(leaf.end_position().row);
        code_since_comment = true;
    }
    found
}

// go/parser junta los comentarios de una tanda (sin código entre ellos): primero los que siguen
// en la misma línea a un token, después los que están en líneas consecutivas.
fn comment_groups<'tree>(comments: &[Comment<'tree>]) -> Vec<Vec<Comment<'tree>>> {
    let mut groups: Vec<Vec<Comment<'tree>>> = Vec::new();
    let mut index = 0;
    while let Some(&first) = comments.get(index) {
        let batch_end = comments
            .iter()
            .skip(index.saturating_add(1))
            .position(|comment| comment.after_code)
            .map_or(comments.len(), |offset| {
                index.saturating_add(1).saturating_add(offset)
            });
        let batch = comments.get(index..batch_end).unwrap_or_default();
        let mut rest = batch;
        if first.trailing {
            let (group, remaining) = take_group(rest, 0);
            groups.push(group);
            rest = remaining;
        }
        while !rest.is_empty() {
            let (group, remaining) = take_group(rest, 1);
            groups.push(group);
            rest = remaining;
        }
        index = batch_end;
    }
    groups
}

fn take_group<'list, 'tree>(
    comments: &'list [Comment<'tree>],
    gap: usize,
) -> (Vec<Comment<'tree>>, &'list [Comment<'tree>]) {
    let mut group = Vec::new();
    let mut end_row = comments.first().map_or(0, |first| first.start_row);
    let mut taken = 0_usize;
    for comment in comments {
        if comment.start_row > end_row.saturating_add(gap) {
            break;
        }
        group.push(*comment);
        end_row = comment.end_row;
        taken = taken.saturating_add(1);
    }
    (group, comments.get(taken..).unwrap_or_default())
}

fn split_block_comments<'tree>(group: &[Comment<'tree>], source: &[u8]) -> Vec<Vec<Comment<'tree>>> {
    let mut visited = Vec::new();
    let mut lines: Vec<Comment<'tree>> = Vec::new();
    for &comment in group {
        if comment_text(comment.node, source).starts_with(BLOCK_START) {
            if !lines.is_empty() {
                visited.push(std::mem::take(&mut lines));
            }
            visited.push(vec![comment]);
        } else {
            lines.push(comment);
        }
    }
    if !lines.is_empty() {
        visited.push(lines);
    }
    visited
}

fn comment_text<'source>(comment: Node<'_>, source: &'source [u8]) -> &'source str {
    comment.utf8_text(source).unwrap_or_default()
}

// CommentGroup.Text() de go/ast, tal cual.
fn group_text(group: &[Comment<'_>], source: &[u8]) -> String {
    let mut lines: Vec<String> = Vec::new();
    for comment in group {
        let text = comment_text(comment.node, source);
        let body = if let Some(line) = text.strip_prefix("//") {
            if line.is_empty() {
                line
            } else if let Some(spaced) = line.strip_prefix(' ') {
                spaced
            } else if is_directive(line) {
                continue;
            } else {
                line
            }
        } else {
            text.strip_prefix("/*")
                .and_then(|inner| inner.strip_suffix("*/"))
                .unwrap_or(text)
        };
        lines.extend(
            body.split('\n')
                .map(|line| line.trim_end_matches(GO_WHITESPACE).to_owned()),
        );
    }
    let mut kept: Vec<String> = Vec::new();
    for line in lines {
        if !line.is_empty() || kept.last().is_some_and(|previous| !previous.is_empty()) {
            kept.push(line);
        }
    }
    if kept.last().is_some_and(|last| !last.is_empty()) {
        kept.push(String::new());
    }
    kept.join("\n")
}

fn is_directive(text: &str) -> bool {
    if DIRECTIVE_PREFIXES.iter().any(|prefix| text.starts_with(prefix)) {
        return true;
    }
    let Some(colon) = text
        .find(':')
        .filter(|&colon| colon > 0 && colon.saturating_add(1) < text.len())
    else {
        return false;
    };
    text.bytes()
        .take(colon.saturating_add(2))
        .enumerate()
        .all(|(index, byte)| index == colon || byte.is_ascii_lowercase() || byte.is_ascii_digit())
}

enum Statement {
    Invalid,
    Permitted,
    Code,
}

fn parsed_body(text: &str) -> Option<(tree_sitter::Tree, String)> {
    let wrapped = format!("{WRAPPER_PREFIX}{text}{WRAPPER_SUFFIX}");
    let tree = parse_snippet(SnippetLanguage::Go, &wrapped)?;
    (!tree.root_node().has_error()).then_some((tree, wrapped))
}

fn body_statements(root: Node<'_>) -> Vec<Node<'_>> {
    let Some(body) = descendants(root)
        .into_iter()
        .find(|node| node.kind() == FUNCTION_DECLARATION)
        .and_then(|function| function.child_by_field_name(BODY))
    else {
        return Vec::new();
    };
    statements_in(body)
}

fn statements_in(block: Node<'_>) -> Vec<Node<'_>> {
    named_children(block)
        .into_iter()
        .flat_map(|child| {
            if child.kind() == "statement_list" {
                named_children(child)
            } else {
                vec![child]
            }
        })
        .filter(|statement| statement.kind() != COMMENT)
        .collect()
}

fn single_statement(text: &str) -> Statement {
    let Some((tree, wrapped)) = parsed_body(text) else {
        return Statement::Invalid;
    };
    match body_statements(tree.root_node()).as_slice() {
        [statement] if is_permitted(*statement, wrapped.as_bytes()) => Statement::Permitted,
        [_] => Statement::Code,
        _ => Statement::Invalid,
    }
}

fn is_code_block(text: &str) -> bool {
    let Some((tree, _)) = parsed_body(&format!("{{ {text} }}")) else {
        return false;
    };
    matches!(body_statements(tree.root_node()).as_slice(),
        [block] if block.kind() == "block" && !statements_in(*block).is_empty())
}

fn is_permitted(statement: Node<'_>, source: &[u8]) -> bool {
    match statement.kind() {
        "expression_statement" => named_children(statement)
            .first()
            .is_some_and(|expression| is_permitted_expression(*expression, source)),
        "labeled_statement" => named_children(statement)
            .into_iter()
            .find(|child| child.kind() != "label_name")
            .is_some_and(|inner| is_permitted(inner, source)),
        "type_declaration" => true,
        _ => false,
    }
}

fn is_permitted_expression(expression: Node<'_>, source: &[u8]) -> bool {
    if CALLS.contains(&expression.kind()) {
        return false;
    }
    if expression.kind() == "unary_expression" {
        return expression
            .child_by_field_name("operator")
            .and_then(|operator| operator.utf8_text(source).ok())
            != Some(RECEIVE);
    }
    true
}
