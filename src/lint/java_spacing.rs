use std::collections::HashSet;

use tree_sitter::Node;

use super::java_tokens::{Lines, Token};
use super::{LintFinding, LintKind, finding};
use crate::fix::Fix;
use crate::rules::JavaRule;
use crate::syntax::{descendants, named_children};

const ASSIGNMENTS: &[&str] = &[
    "=", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "<<=", ">>=", ">>>=",
];
const BINARY_OPERATORS: &[&str] = &[
    "+", "-", "*", "/", "%", "<<", ">>", ">>>", "==", "!=", "<", ">", "<=", ">=", "&&", "||", "^",
];
const KEYWORDS_AROUND: &[&str] = &[
    "if",
    "else",
    "for",
    "while",
    "do",
    "try",
    "catch",
    "finally",
    "switch",
    "synchronized",
    "return",
    "assert",
];
const KEYWORDS_AFTER: &[&str] = &[
    "if",
    "else",
    "while",
    "do",
    "for",
    "finally",
    "return",
    "yield",
    "catch",
    "try",
    "switch",
    "synchronized",
    "case",
    "when",
];
const BITWISE_OR_TYPE_AND: &[&str] = &["&", "|"];
const BINARY: &str = "binary_expression";
const TERNARY: &str = "ternary_expression";
const QUESTION: &str = "?";
const COLON: &str = ":";
const COLON_OWNERS: &[&str] = &[TERNARY, "assert_statement"];
const ARROW: &str = "->";
const OPEN_CURLY: &str = "{";
const CLOSE_CURLY: &str = "}";
const ARRAY_INITIALIZERS: &[&str] = &["array_initializer", "element_value_array_initializer"];
const CLASS_BODY: &str = "class_body";
const BLOCK: &str = "block";
const RETURN: &str = "return";
const COMMA: &str = ",";
const SEMICOLON: &str = ";";
const CLOSE_PAREN: &str = ")";
const OPEN_PAREN: &str = "(";
const CAST: &str = "cast_expression";
const ELLIPSIS: &str = "...";
const UPDATES: &[&str] = &["++", "--"];
const UPDATE: &str = "update_expression";
const LABELED: &str = "labeled_statement";
const FOR: &str = "for_statement";
const CLOSE_CURLY_FOLLOWERS: &[u8] = b"),;.";
const AFTER_EXCEPTIONS: &[u8] = b";)";
const COMMENTS: &[&str] = &["line_comment", "block_comment"];
const SPACE: &str = " ";
const DOT: &str = ".";

pub fn reported(
    nodes: &[Node<'_>],
    tokens: &[Token<'_>],
    lines: &Lines<'_>,
    source: &[u8],
) -> Vec<LintFinding> {
    let mut findings: Vec<LintFinding> = annotated_ellipses(nodes)
        .into_iter()
        .filter_map(|ellipsis| before(ellipsis, ELLIPSIS, lines, source))
        .collect();
    let mut previous: Option<Token<'_>> = None;
    for token in tokens.iter().copied().filter(|token| !token.is_comment()) {
        if checked_around(&token) {
            findings.extend(around(&token, lines, source));
        }
        if checked_after(&token) {
            findings.extend(after(&token, lines, source));
        }
        if checked_before(&token, previous.as_ref()) {
            findings.extend(before(token.node, token.kind(), lines, source));
        }
        previous = Some(token);
    }
    findings
}

fn checked_around(token: &Token<'_>) -> bool {
    let kind = token.kind();
    if ASSIGNMENTS.contains(&kind) || BITWISE_OR_TYPE_AND.contains(&kind) || KEYWORDS_AROUND.contains(&kind) {
        return true;
    }
    match kind {
        QUESTION => token.parent_kind() == TERNARY,
        COLON => COLON_OWNERS.contains(&token.parent_kind()),
        ARROW => true,
        OPEN_CURLY | CLOSE_CURLY => !ARRAY_INITIALIZERS.contains(&token.parent_kind()),
        _ => BINARY_OPERATORS.contains(&kind) && token.parent_kind() == BINARY,
    }
}

// Calca WhitespaceAround con las opciones de sun_checks: ningún bloque vacío se perdona. Un `}`
// seguido de `)`, `,`, `;` o `.` y un `return` sin valor no piden espacio después, y la llave de
// un bloque de inicialización pegada a la de su clase (`{{ ... }}`) no pide espacio entre las dos.
fn around(token: &Token<'_>, lines: &Lines<'_>, source: &[u8]) -> Option<LintFinding> {
    let start = token.start();
    let end = token.end();
    let missing_before =
        start.column > 0 && !lines.whitespace_before(start) && !opens_or_closes_after_initializer(token);
    let missing_after =
        end.column < lines.width(end.row) && !lines.whitespace_at(end) && !exempt_after(token, lines);
    if !missing_before && !missing_after {
        return None;
    }
    let text = token.kind();
    let padded = format!(
        "{}{text}{}",
        if missing_before { SPACE } else { "" },
        if missing_after { SPACE } else { "" }
    );
    Some(finding(
        LintKind::Java(JavaRule::WhitespaceAround),
        token.node,
        text,
        Some(Fix::replace(token.node, &padded, source)),
    ))
}

fn exempt_after(token: &Token<'_>, lines: &Lines<'_>) -> bool {
    let end = token.end();
    match token.kind() {
        RETURN => token.parent.named_child_count() == 0,
        CLOSE_CURLY => {
            lines
                .byte(end.row, end.column)
                .is_some_and(|next| CLOSE_CURLY_FOLLOWERS.contains(&next))
                || initializer_closes_class(token)
        }
        OPEN_CURLY => {
            token.parent_kind() == CLASS_BODY && first_member(token.parent).is_some_and(is_initializer)
        }
        _ => false,
    }
}

fn opens_or_closes_after_initializer(token: &Token<'_>) -> bool {
    match token.kind() {
        OPEN_CURLY => token.parent_kind() == BLOCK && token.grandparent_kind() == CLASS_BODY,
        CLOSE_CURLY => {
            token.parent_kind() == CLASS_BODY && last_member(token.parent).is_some_and(is_initializer)
        }
        _ => false,
    }
}

fn initializer_closes_class(token: &Token<'_>) -> bool {
    token.parent_kind() == BLOCK
        && token
            .grandparent
            .filter(|class_body| class_body.kind() == CLASS_BODY)
            .is_some_and(|class_body| last_member(class_body) == Some(token.parent))
}

fn is_initializer(member: Node<'_>) -> bool {
    member.kind() == BLOCK
}

fn members(class_body: Node<'_>) -> Vec<Node<'_>> {
    named_children(class_body)
        .into_iter()
        .filter(|member| !COMMENTS.contains(&member.kind()))
        .collect()
}

fn first_member(class_body: Node<'_>) -> Option<Node<'_>> {
    members(class_body).into_iter().next()
}

fn last_member(class_body: Node<'_>) -> Option<Node<'_>> {
    members(class_body).into_iter().last()
}

fn checked_after(token: &Token<'_>) -> bool {
    let kind = token.kind();
    match kind {
        COMMA | ELLIPSIS | ARROW => true,
        SEMICOLON => !token.is_empty_statement(),
        CLOSE_PAREN => token.parent_kind() == CAST,
        _ => KEYWORDS_AFTER.contains(&kind),
    }
}

// Calca WhitespaceAfter: después de la coma, el `;`, el cierre de un cast, `->`, `...` y las
// palabras de control va un blanco, salvo al final de la línea o antes de `;` o `)`.
fn after(token: &Token<'_>, lines: &Lines<'_>, source: &[u8]) -> Option<LintFinding> {
    let end = token.end();
    let next = lines.byte(end.row, end.column)?;
    if super::java_tokens::is_whitespace(next) || AFTER_EXCEPTIONS.contains(&next) {
        return None;
    }
    Some(finding(
        LintKind::Java(JavaRule::WhitespaceAfter),
        token.node,
        token.kind(),
        Some(Fix::insert_at(end, SPACE, source)),
    ))
}

fn checked_before(token: &Token<'_>, previous: Option<&Token<'_>>) -> bool {
    match token.kind() {
        COMMA | ELLIPSIS => true,
        SEMICOLON => !token.is_empty_statement() && !closes_empty_for_clause(token, previous),
        COLON => token.parent_kind() == LABELED,
        kind if UPDATES.contains(&kind) => {
            token.parent_kind() == UPDATE && token.parent.start_byte() < token.node.start_byte()
        }
        _ => false,
    }
}

fn closes_empty_for_clause(token: &Token<'_>, previous: Option<&Token<'_>>) -> bool {
    token.parent_kind() == FOR
        && previous.is_some_and(|previous| {
            previous.kind() == SEMICOLON || (previous.kind() == OPEN_PAREN && previous.parent == token.parent)
        })
}

// Calca NoWhitespaceBefore sin cortes de línea permitidos: delante de la coma, el `;`, `...`, el
// `++`/`--` de después y los dos puntos de una etiqueta no va blanco, ni siquiera al principio de
// la línea. El arreglo sólo junta lo que está en la misma línea.
fn before(node: Node<'_>, subject: &str, lines: &Lines<'_>, source: &[u8]) -> Option<LintFinding> {
    let start = node.start_position();
    if start.column > 0 && !lines.whitespace_before(start) {
        return None;
    }
    let fix = (!lines.is_blank_before(start))
        .then(|| Fix::replace_range(lines.blank_start(start), start, "", source));
    Some(finding(
        LintKind::Java(JavaRule::NoWhitespaceBefore),
        node,
        subject,
        fix,
    ))
}

// tree-sitter no entiende una anotación sobre `...` (`Object @Nullable ... args`) y deja los tres
// puntos sueltos dentro de un ERROR: se reconocen ahí para mirar el blanco de antes igual.
fn annotated_ellipses<'tree>(nodes: &[Node<'tree>]) -> Vec<Node<'tree>> {
    let mut seen = HashSet::new();
    let mut found = Vec::new();
    for error in nodes.iter().filter(|node| node.is_error()) {
        let dots: Vec<Node<'tree>> = descendants(*error)
            .into_iter()
            .filter(|node| node.kind() == DOT && node.child_count() == 0)
            .collect();
        for window in dots.windows(ELLIPSIS.len()) {
            if let [first, second, third] = window
                && first.end_byte() == second.start_byte()
                && second.end_byte() == third.start_byte()
                && seen.insert(first.id())
            {
                found.push(*first);
            }
        }
    }
    found
}
