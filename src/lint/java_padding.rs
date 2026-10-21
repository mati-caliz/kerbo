use std::collections::HashSet;

use tree_sitter::{Node, Point};

use super::java::children;
use super::java_tokens::{Lines, Token, is_whitespace};
use super::{LintFinding, LintKind, finding, finding_at};
use crate::fix::Fix;
use crate::rules::JavaRule;
use crate::syntax::named_children;

const AT: &str = "@";
const ANNOTATIONS: &[&str] = &["marker_annotation", "annotation"];
const UPDATES: &[&str] = &["++", "--"];
const UPDATE: &str = "update_expression";
const UNARY_OPERATORS: &[&str] = &["-", "+", "~", "!"];
const UNARY: &str = "unary_expression";
const DOT: &str = ".";
const OPEN_CURLY: &str = "{";
const ARRAY_INITIALIZER: &str = "array_initializer";
const OPEN_BRACKET: &str = "[";
const BRACKET_OWNERS: &[&str] = &["dimensions", "dimensions_expr", "array_access"];
const OPEN_ANGLE: &str = "<";
const CLOSE_ANGLE: &str = ">";
const GENERIC_OWNERS: &[&str] = &["type_arguments", "type_parameters"];
const TYPE_PARAMETERS: &str = "type_parameters";
const TYPE_ARGUMENTS: &str = "type_arguments";
const GENERIC_DECLARATIONS: &[&str] = &["method_declaration", "constructor_declaration"];
const INVOCATIONS: &[&str] = &[
    "object_creation_expression",
    "method_invocation",
    "explicit_constructor_invocation",
    "method_reference",
];
const GENERIC_TYPE: &str = "generic_type";
const TYPE_FIELD: &str = "type";
const VALID_AFTER_GENERIC: &[u8] = b"),[.:;";
const AMPERSAND: u8 = b'&';
const SPACE_BYTE: u8 = b' ';
const SPACE: &str = " ";
const OPEN_PAREN: &str = "(";
const PARAMETERS: &str = "formal_parameters";
const PARAMETER_OWNERS: &[&str] = &[
    "method_declaration",
    "constructor_declaration",
    "record_declaration",
];
const ARGUMENTS: &str = "argument_list";
const ARGUMENT_OWNERS: &[&str] = &[
    "method_invocation",
    "object_creation_expression",
    "explicit_constructor_invocation",
    "enum_constant",
];
const FOR: &str = "for_statement";
const UPDATE_FIELD: &str = "update";
const BODY_FIELD: &str = "body";
const SEMICOLON: &str = ";";
const CLOSE_PAREN: u8 = b')';

pub fn reported(
    nodes: &[Node<'_>],
    tokens: &[Token<'_>],
    lines: &Lines<'_>,
    source: &[u8],
) -> Vec<LintFinding> {
    let invoked = invocation_type_arguments(nodes);
    let mut findings: Vec<LintFinding> = nodes
        .iter()
        .filter(|node| node.kind() == FOR)
        .filter_map(|node| empty_iterator_pad(*node, lines, source))
        .collect();
    let mut depth = 0_usize;
    let mut previous: Option<Token<'_>> = None;
    for token in tokens.iter().copied().filter(|token| !token.is_comment()) {
        findings.extend(whitespace_after(&token, previous.as_ref(), lines, source));
        findings.extend(method_parameter_pad(&token, lines, source));
        if GENERIC_OWNERS.contains(&token.parent_kind()) {
            match token.kind() {
                OPEN_ANGLE => {
                    findings.extend(generic_start(&token, lines, source));
                    depth = depth.saturating_add(1);
                }
                CLOSE_ANGLE => {
                    let closing = Closing {
                        depth,
                        in_call: invoked.contains(&token.parent.id()),
                    };
                    findings.extend(generic_end(&token, closing, lines, source));
                    depth = depth.saturating_sub(1);
                }
                _ => {}
            }
        }
        previous = Some(token);
    }
    findings
}

// Calca NoWhitespaceAfter: detrás de `@`, `++`/`--` de antes, los signos unarios, `!`, `~`, el punto
// y la llave de un inicializador de arreglo no va blanco, salvo un corte de línea. El `[` de un
// arreglo o un índice mira el blanco entre lo anterior y él.
fn whitespace_after(
    token: &Token<'_>,
    previous: Option<&Token<'_>>,
    lines: &Lines<'_>,
    source: &[u8],
) -> Option<LintFinding> {
    if token.kind() == OPEN_BRACKET && BRACKET_OWNERS.contains(&token.parent_kind()) {
        let previous = previous.filter(|previous| {
            previous.end().row == token.start().row && !ANNOTATIONS.contains(&previous.parent_kind())
        })?;
        return lines.whitespace_at(previous.end()).then(|| {
            let fix = Fix::replace_range(previous.end(), token.start(), "", source);
            finding(
                LintKind::Java(JavaRule::NoWhitespaceAfter),
                token.node,
                previous.kind(),
                Some(fix),
            )
        });
    }
    if !checked_after(token) {
        return None;
    }
    let end = token.end();
    (end.column < lines.width(end.row) && lines.whitespace_at(end)).then(|| {
        let fix = Fix::replace_range(end, lines.blank_end(end), "", source);
        finding(
            LintKind::Java(JavaRule::NoWhitespaceAfter),
            token.node,
            token.kind(),
            Some(fix),
        )
    })
}

fn checked_after(token: &Token<'_>) -> bool {
    match token.kind() {
        AT => ANNOTATIONS.contains(&token.parent_kind()),
        DOT => true,
        OPEN_CURLY => token.parent_kind() == ARRAY_INITIALIZER,
        kind if UPDATES.contains(&kind) => {
            token.parent_kind() == UPDATE && token.parent.start_byte() == token.node.start_byte()
        }
        kind => UNARY_OPERATORS.contains(&kind) && token.parent_kind() == UNARY,
    }
}

// Los argumentos de tipo de una llamada (`Foo.<T>bar()`, `new Foo<T>()`, `Foo::<T>bar`): Checkstyle
// sólo les pide que no haya blanco después del `>`.
fn invocation_type_arguments(nodes: &[Node<'_>]) -> HashSet<usize> {
    let mut found = HashSet::new();
    for node in nodes.iter().filter(|node| INVOCATIONS.contains(&node.kind())) {
        found.extend(
            named_children(*node)
                .into_iter()
                .filter(|child| child.kind() == TYPE_ARGUMENTS)
                .map(|child| child.id()),
        );
        found.extend(
            node.child_by_field_name(TYPE_FIELD)
                .filter(|written| written.kind() == GENERIC_TYPE)
                .map(named_children)
                .unwrap_or_default()
                .into_iter()
                .filter(|child| child.kind() == TYPE_ARGUMENTS)
                .map(|child| child.id()),
        );
    }
    found
}

// Calca GenericWhitespace para `<`: pide blanco antes sólo en los parámetros de tipo de un método o
// constructor, no lo quiere antes en el resto (salvo a principio de línea) y nunca después.
fn generic_start(token: &Token<'_>, lines: &Lines<'_>, source: &[u8]) -> Option<LintFinding> {
    let start = token.start();
    let end = token.end();
    let mut edits_before = None;
    if start.column > 0 {
        let declares = token.parent_kind() == TYPE_PARAMETERS
            && GENERIC_DECLARATIONS.contains(&token.grandparent_kind());
        if declares && !lines.whitespace_before(start) {
            edits_before = Some(Fix::insert_at(start, SPACE, source));
        } else if !declares && lines.whitespace_before(start) && !lines.is_blank_before(start) {
            edits_before = Some(Fix::replace_range(lines.blank_start(start), start, "", source));
        }
    }
    let followed = lines
        .whitespace_at(end)
        .then(|| Fix::replace_range(end, lines.blank_end(end), "", source));
    edits_before.or(followed).map(|fix| {
        finding(
            LintKind::Java(JavaRule::GenericWhitespace),
            token.node,
            OPEN_ANGLE,
            Some(fix),
        )
    })
}

// Para `>`: sin blanco antes (salvo a principio de línea); el último de un genérico va seguido de
// blanco o de `)`, `,`, `[`, `.`, `:` o `;`, y en una llamada de nada en blanco; uno anidado no
// lleva blanco después, salvo uno solo antes de un `&`.
#[derive(Clone, Copy)]
struct Closing {
    depth: usize,
    in_call: bool,
}

fn generic_end(token: &Token<'_>, closing: Closing, lines: &Lines<'_>, source: &[u8]) -> Option<LintFinding> {
    let start = token.start();
    let end = token.end();
    let report = |fix: Fix| {
        finding(
            LintKind::Java(JavaRule::GenericWhitespace),
            token.node,
            CLOSE_ANGLE,
            Some(fix),
        )
    };
    if start.column > 0 && lines.whitespace_before(start) && !lines.is_blank_before(start) {
        return Some(report(Fix::replace_range(
            lines.blank_start(start),
            start,
            "",
            source,
        )));
    }
    let next = lines.byte(end.row, end.column)?;
    let remove_blank = || Fix::replace_range(end, lines.blank_end(end), "", source);
    if closing.depth > 1 {
        return nested_end(token, lines).map(|problem| {
            report(match problem {
                NestedProblem::MissingBlank => Fix::insert_at(end, SPACE, source),
                NestedProblem::ExtraBlank => remove_blank(),
            })
        });
    }
    if closing.in_call {
        return is_whitespace(next).then(|| report(remove_blank()));
    }
    (!is_whitespace(next) && !VALID_AFTER_GENERIC.contains(&next))
        .then(|| report(Fix::insert_at(end, SPACE, source)))
}

enum NestedProblem {
    MissingBlank,
    ExtraBlank,
}

fn nested_end(token: &Token<'_>, lines: &Lines<'_>) -> Option<NestedProblem> {
    let end = token.end();
    let width = lines.width(end.row);
    let ampersand = (end.column..width).find(|column| lines.byte(end.row, *column) == Some(AMPERSAND));
    if let Some(ampersand) = ampersand
        && (end.column..ampersand).all(|column| lines.byte(end.row, column).is_some_and(is_whitespace))
    {
        return match ampersand.saturating_sub(end.column) {
            0 => Some(NestedProblem::MissingBlank),
            1 => None,
            _ => Some(NestedProblem::ExtraBlank),
        };
    }
    (lines.byte(end.row, end.column) == Some(SPACE_BYTE)).then_some(NestedProblem::ExtraBlank)
}

// Calca MethodParamPad: el `(` de una declaración o una llamada va pegado al nombre, y tampoco puede
// empezar la línea.
fn method_parameter_pad(token: &Token<'_>, lines: &Lines<'_>, source: &[u8]) -> Option<LintFinding> {
    let padded = token.kind() == OPEN_PAREN
        && match token.parent_kind() {
            PARAMETERS => PARAMETER_OWNERS.contains(&token.grandparent_kind()),
            ARGUMENTS => ARGUMENT_OWNERS.contains(&token.grandparent_kind()),
            _ => false,
        };
    if !padded {
        return None;
    }
    let start = token.start();
    let fix = if lines.is_blank_before(start) {
        None
    } else if lines.whitespace_before(start) {
        Some(Fix::replace_range(lines.blank_start(start), start, "", source))
    } else {
        return None;
    };
    Some(finding(
        LintKind::Java(JavaRule::MethodParamPad),
        token.node,
        OPEN_PAREN,
        fix,
    ))
}

// Calca EmptyForIteratorPad: en un `for` sin avance, después del último `;` no va blanco. Checkstyle
// marca lo primero que sigue al blanco.
fn empty_iterator_pad(for_statement: Node<'_>, lines: &Lines<'_>, source: &[u8]) -> Option<LintFinding> {
    if for_statement.child_by_field_name(UPDATE_FIELD).is_some() {
        return None;
    }
    let body = for_statement.child_by_field_name(BODY_FIELD);
    let semicolon = children(for_statement)
        .into_iter()
        .rfind(|child| child.kind() == SEMICOLON && Some(*child) != body)?;
    let after = semicolon.end_position();
    if after.column >= lines.width(after.row) || !lines.whitespace_at(after) {
        return None;
    }
    let blank_end = lines.blank_end(after);
    let fix = (lines.byte(blank_end.row, blank_end.column) == Some(CLOSE_PAREN))
        .then(|| Fix::replace_range(after, blank_end, "", source));
    Some(finding_at(
        LintKind::Java(JavaRule::EmptyForIteratorPad),
        Point::new(blank_end.row, blank_end.column),
        SEMICOLON,
        fix,
    ))
}
