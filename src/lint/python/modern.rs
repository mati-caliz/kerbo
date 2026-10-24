use std::sync::LazyLock;

use regex::Regex;
use tree_sitter::{Node, Point};

use super::Report;
use super::ast::{
    ATTRIBUTE, ATTRIBUTE_FIELD, BLOCK, CALL, CLASS, COMMENT, COMPARISON, DECORATED, EXPRESSION_STATEMENT,
    FUNCTION, FUNCTION_FIELD, IDENTIFIER, MODULE, OBJECT_FIELD, STRING, named, string_prefix, text,
    unparenthesized,
};
use crate::rules::PythonRule;

const UNICODE_PREFIX: char = 'u';
const SELF: &str = "self";
const DEPRECATED_UNITTEST_ALIASES: [&str; 15] = [
    "assertAlmostEquals",
    "assertEquals",
    "assertNotAlmostEquals",
    "assertNotEquals",
    "assertNotRegexpMatches",
    "assertRaisesRegexp",
    "assertRegexpMatches",
    "assert_",
    "failIf",
    "failIfAlmostEqual",
    "failIfEqual",
    "failUnless",
    "failUnlessAlmostEqual",
    "failUnlessEqual",
    "failUnlessRaises",
];
const PASS: &str = "pass_statement";
const ELLIPSIS: &str = "ellipsis";
const IF: &str = "if_statement";
const CONDITION_FIELD: &str = "condition";
const CONSEQUENCE_FIELD: &str = "consequence";
const SUPERCLASSES_FIELD: &str = "superclasses";
const DECORATOR: &str = "decorator";
const TYPE_CHECKING: &str = "TYPE_CHECKING";
const PROTOCOL: &str = "Protocol";
const ABSTRACT_DECORATORS: [&str; 4] = [
    "abstractmethod",
    "abstractproperty",
    "abstractclassmethod",
    "abstractstaticmethod",
];
const UTF8_NAMES: [&str; 2] = ["utf8", "utf-8"];
const MAX_CODING_LINE: usize = 1;
const MIN_PLACEHOLDER_SIBLINGS: usize = 2;
const FUTURE_IMPORT: &str = "future_import_statement";
const ALIASED_IMPORT: &str = "aliased_import";
const NAME_FIELD: &str = "name";
const MANDATORY_FUTURE_FEATURES: [&str; 8] = [
    "nested_scopes",
    "generators",
    "with_statement",
    "division",
    "absolute_import",
    "print_function",
    "unicode_literals",
    "generator_stop",
];

static CODING_COMMENT: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"^[ \t\f]*#.*?coding[:=][ \t]*(?<name>[-_.a-zA-Z0-9]+)").ok());

#[derive(Clone, Copy, PartialEq, Eq)]
enum Coding {
    Utf8(Point),
    Other,
    Absent,
}

pub fn reports(root: Node<'_>, nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    let mut found: Vec<Report> = coding_comment(root, source).into_iter().collect();
    for node in nodes {
        match node.kind() {
            STRING
                if string_prefix(*node, source)
                    .to_ascii_lowercase()
                    .contains(UNICODE_PREFIX) =>
            {
                found.push(Report::at(PythonRule::UP025, *node, ""));
            }
            CALL => found.extend(deprecated_unittest_alias(*node, source)),
            FUTURE_IMPORT => found.extend(unnecessary_future_import(*node, source)),
            EXPRESSION_STATEMENT => found.extend(useless_comparison(*node)),
            BLOCK | MODULE => found.extend(unnecessary_placeholders(*node, source)),
            _ => {}
        }
    }
    found
}

// UP010: un `from __future__ import` de algo que ya es obligatorio en Python 3. Avisa una vez por
// sentencia.
fn unnecessary_future_import(statement: Node<'_>, source: &[u8]) -> Option<Report> {
    let mut cursor = statement.walk();
    let mandatory = statement
        .children_by_field_name(NAME_FIELD, &mut cursor)
        .filter_map(|name| {
            if name.kind() == ALIASED_IMPORT {
                name.child_by_field_name(NAME_FIELD)
            } else {
                Some(name)
            }
        })
        .any(|name| MANDATORY_FUTURE_FEATURES.contains(&text(name, source)));
    mandatory.then(|| Report::at(PythonRule::UP010, statement, ""))
}

// UP009: un `# -*- coding: utf-8 -*-` en las dos primeras líneas, que en Python 3 sobra.
fn coding_comment(root: Node<'_>, source: &[u8]) -> Option<Report> {
    let codings = leading_codings(root, source);
    let mut remaining = codings
        .iter()
        .copied()
        .skip_while(|coding| *coding == Coding::Absent);
    let Some(Coding::Utf8(start)) = remaining.next() else {
        return None;
    };
    if start.row > MAX_CODING_LINE {
        return None;
    }
    let other_follows = matches!(
        (remaining.next(), remaining.next()),
        (Some(Coding::Other), _) | (Some(Coding::Absent), Some(Coding::Other))
    );
    (!other_follows).then(|| Report::at_point(PythonRule::UP009, start, ""))
}

// Los comentarios del principio del archivo, cada uno sólo en su línea y sin código antes.
fn leading_codings(root: Node<'_>, source: &[u8]) -> Vec<Coding> {
    let mut codings = Vec::new();
    let mut previous_end = 0;
    for comment in comments_in_order(root) {
        let before = source.get(previous_end..comment.start_byte()).unwrap_or_default();
        if !String::from_utf8_lossy(before).trim().is_empty() {
            break;
        }
        previous_end = comment.end_byte();
        let line_start = source
            .get(..comment.start_byte())
            .and_then(|head| head.iter().rposition(|byte| *byte == b'\n'))
            .map_or(0, |newline| newline.saturating_add(1));
        let line = String::from_utf8_lossy(source.get(line_start..comment.end_byte()).unwrap_or_default())
            .into_owned();
        let name = CODING_COMMENT
            .as_ref()
            .and_then(|pattern| pattern.captures(&line))
            .and_then(|captures| captures.name("name").map(|name| name.as_str().to_owned()));
        codings.push(match name {
            Some(name) if UTF8_NAMES.contains(&name.as_str()) => Coding::Utf8(comment.start_position()),
            Some(_) => Coding::Other,
            None => Coding::Absent,
        });
    }
    codings
}

fn comments_in_order(root: Node<'_>) -> Vec<Node<'_>> {
    let mut comments = Vec::new();
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if node.kind() == COMMENT {
            comments.push(node);
            continue;
        }
        let mut cursor = node.walk();
        pending.extend(node.children(&mut cursor));
    }
    comments.sort_by_key(Node::start_byte);
    comments
}

// UP005: `self.assertEquals(...)` y los demás alias que `unittest` sacó en Python 3.12.
fn deprecated_unittest_alias(call: Node<'_>, source: &[u8]) -> Option<Report> {
    let function = call
        .child_by_field_name(FUNCTION_FIELD)
        .filter(|function| function.kind() == ATTRIBUTE)?;
    let is_self = function
        .child_by_field_name(OBJECT_FIELD)
        .is_some_and(|object| object.kind() == IDENTIFIER && text(object, source) == SELF);
    let alias = function
        .child_by_field_name(ATTRIBUTE_FIELD)
        .map(|attribute| text(attribute, source))?;
    (is_self && DEPRECATED_UNITTEST_ALIASES.contains(&alias))
        .then(|| Report::at(PythonRule::UP005, function, alias))
}

// B015: una comparación suelta como sentencia, cuyo resultado se descarta.
fn useless_comparison(statement: Node<'_>) -> Option<Report> {
    let parts = named(statement);
    let [expression] = parts.as_slice() else {
        return None;
    };
    let expression = unparenthesized(*expression);
    (expression.kind() == COMPARISON).then(|| Report::at(PythonRule::B015, expression, ""))
}

// PIE790: un `pass` o un `...` en un bloque que ya tiene otra sentencia.
fn unnecessary_placeholders(block: Node<'_>, source: &[u8]) -> Vec<Report> {
    let statements: Vec<Node<'_>> = named(block)
        .into_iter()
        .filter(|child| child.kind() != COMMENT)
        .collect();
    if statements.len() < MIN_PLACEHOLDER_SIBLINGS {
        return Vec::new();
    }
    let mut found = Vec::new();
    for statement in statements {
        if statement.kind() == PASS {
            found.push(Report::at(PythonRule::PIE790, statement, ""));
        } else if is_ellipsis_statement(statement) {
            if in_type_checking_block(block, source) || in_protocol_or_abstract_method(block, source) {
                return found;
            }
            found.push(Report::at(PythonRule::PIE790, statement, ""));
        }
    }
    found
}

fn is_ellipsis_statement(statement: Node<'_>) -> bool {
    statement.kind() == EXPRESSION_STATEMENT
        && matches!(named(statement).as_slice(), [only] if only.kind() == ELLIPSIS)
}

fn in_type_checking_block(block: Node<'_>, source: &[u8]) -> bool {
    std::iter::successors(Some(block), Node::parent).any(|ancestor| {
        ancestor.kind() == IF
            && ancestor
                .child_by_field_name(CONSEQUENCE_FIELD)
                .is_some_and(|body| {
                    body.start_byte() <= block.start_byte() && block.end_byte() <= body.end_byte()
                })
            && ancestor
                .child_by_field_name(CONDITION_FIELD)
                .is_some_and(|condition| ends_with_name(text(condition, source), TYPE_CHECKING))
    })
}

fn in_protocol_or_abstract_method(block: Node<'_>, source: &[u8]) -> bool {
    let Some(function) = block.parent().filter(|parent| parent.kind() == FUNCTION) else {
        return false;
    };
    let decorators = function
        .parent()
        .filter(|parent| parent.kind() == DECORATED)
        .map_or_else(Vec::new, |decorated| {
            named(decorated)
                .into_iter()
                .filter(|child| child.kind() == DECORATOR)
                .collect()
        });
    let abstract_method = decorators.iter().any(|decorator| {
        let name = text(*decorator, source).trim_start_matches('@').trim();
        ABSTRACT_DECORATORS
            .iter()
            .any(|abstract_name| ends_with_name(name, abstract_name))
    });
    abstract_method || enclosing_class(function).is_some_and(|class| is_protocol(class, source))
}

fn enclosing_class(function: Node<'_>) -> Option<Node<'_>> {
    let holder = function
        .parent()
        .filter(|parent| parent.kind() == DECORATED)
        .unwrap_or(function);
    holder
        .parent()
        .filter(|parent| parent.kind() == BLOCK)
        .and_then(|block| block.parent())
        .filter(|class| class.kind() == CLASS)
}

fn is_protocol(class: Node<'_>, source: &[u8]) -> bool {
    class
        .child_by_field_name(SUPERCLASSES_FIELD)
        .is_some_and(|bases| {
            named(bases).into_iter().any(|base| {
                let name = text(base, source);
                let name = name.split('[').next().unwrap_or(name);
                ends_with_name(name, PROTOCOL)
            })
        })
}

fn ends_with_name(dotted: &str, name: &str) -> bool {
    dotted == name || dotted.strip_suffix(name).is_some_and(|head| head.ends_with('.'))
}
