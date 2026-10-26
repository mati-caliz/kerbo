mod shape;

use tree_sitter::Node;

use super::Report;
use super::ast::{COMMENT, DECORATED, NAME_FIELD, named, text};
use super::fix_edits::delete_comment;
use crate::rules::PythonRule;
use shape::{Item, is_first_in_alternate_body, is_statement, items, statements};

const HASH: char = '#';
const NEWLINE: u8 = b'\n';
const BACKSLASH: u8 = b'\\';
const HASH_BYTE: u8 = b'#';
const TRIVIA: &[u8] = b" \t\x0c\r\n";
const INDENTATION: &[u8] = b" \t\x0c";
const FORMAT_PREFIX: &str = "fmt:";
const YAPF_PREFIX: &str = "yapf:";
const DECORATOR: &str = "decorator";
const DEFINITION_FIELD: &str = "definition";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Off,
    On,
    Skip,
}

#[derive(Clone, Copy)]
struct Suppression<'tree> {
    node: Node<'tree>,
    kind: Kind,
}

// Lo que ruff sabe de un comentario cuando lo ubica: el nodo que lo encierra, el anterior y el
// siguiente (`SuppressionCommentData`).
struct Placement<'tree> {
    enclosing: Option<Node<'tree>>,
    preceding: Option<Node<'tree>>,
    following: Option<Node<'tree>>,
}

struct Walker<'tree, 'source> {
    source: &'source [u8],
    comments: Vec<Node<'tree>>,
    pending: Vec<Suppression<'tree>>,
    next: usize,
    parents: Vec<Node<'tree>>,
    preceding: Option<Node<'tree>>,
    reports: Vec<Report>,
}

// RUF028: un `# fmt: off`, `# fmt: on`, `# fmt: skip` (o `# yapf: disable`/`enable`) donde el
// formateador de ruff no lo toma. Recorre el árbol como el `SuppressionCommentVisitor` de ruff.
pub fn reports(root: Node<'_>, nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    let mut comments: Vec<Node<'_>> = nodes
        .iter()
        .filter(|node| node.kind() == COMMENT)
        .copied()
        .collect();
    comments.sort_by_key(Node::start_byte);
    let pending: Vec<Suppression<'_>> = comments
        .iter()
        .filter_map(|node| {
            suppression_kind(text(*node, source)).map(|kind| Suppression { node: *node, kind })
        })
        .collect();
    if pending.is_empty() {
        return Vec::new();
    }
    let mut walker = Walker {
        source,
        comments,
        pending,
        next: 0,
        parents: Vec::new(),
        preceding: None,
        reports: Vec::new(),
    };
    walker.visit_body(&statements(root));
    walker.reports
}

// `SuppressionKind::from_comment` de ruff.
fn suppression_kind(comment: &str) -> Option<Kind> {
    let trimmed = comment.strip_prefix(HASH).unwrap_or(comment).trim();
    if let Some(command) = trimmed.strip_prefix(FORMAT_PREFIX) {
        match command.trim_start() {
            "off" => return Some(Kind::Off),
            "on" => return Some(Kind::On),
            "skip" => return Some(Kind::Skip),
            _ => {}
        }
    } else if let Some(command) = trimmed.strip_prefix(YAPF_PREFIX) {
        match command.trim_start() {
            "disable" => return Some(Kind::Off),
            "enable" => return Some(Kind::On),
            _ => {}
        }
    }
    comment
        .split(HASH)
        .filter_map(|segment| segment.trim().strip_prefix(FORMAT_PREFIX))
        .any(|command| command.trim_start() == "skip")
        .then_some(Kind::Skip)
}

impl<'tree> Walker<'tree, '_> {
    fn peek(&self) -> Option<Suppression<'tree>> {
        self.pending.get(self.next).copied()
    }

    fn can_skip(&self, end: usize) -> bool {
        self.peek().is_none_or(|comment| comment.node.start_byte() >= end)
    }

    fn visit_body(&mut self, body: &[Node<'tree>]) {
        let (Some(first), Some(last)) = (body.first(), body.last()) else {
            return;
        };
        if body.len() > 1 && self.can_skip(last.end_byte()) {
            self.visit(*first);
            self.preceding = Some(*last);
            return;
        }
        for statement in body {
            self.visit(*statement);
        }
    }

    fn visit(&mut self, node: Node<'tree>) {
        let enclosing = self.parents.last().copied();
        while let Some(comment) = self.peek() {
            if comment.node.end_byte() > node.start_byte() {
                break;
            }
            let placement = Placement {
                enclosing,
                preceding: self.preceding,
                following: Some(node),
            };
            self.capture(comment, &placement);
        }
        self.preceding = None;
        self.parents.push(node);
        if !self.can_skip(node.end_byte()) {
            for item in items(node) {
                match item {
                    Item::Node(child) => self.visit(child),
                    Item::Body(body) => self.visit_body(&body),
                }
            }
        }
        self.parents.pop();
        self.leave(node);
        self.preceding = Some(node);
    }

    // Los comentarios que quedan adentro del nodo, y los que vienen justo después en la misma línea
    // (o en otra más indentada), van con el nodo.
    fn leave(&mut self, node: Node<'tree>) {
        while let Some(comment) = self.peek() {
            if self.is_past(node, comment.node) {
                break;
            }
            let placement = Placement {
                enclosing: Some(node),
                preceding: self.preceding,
                following: None,
            };
            self.capture(comment, &placement);
        }
    }

    // Un comentario después del nodo que no le pertenece: hay código en el medio, o está en su propia
    // línea con una indentación que no es mayor que la del nodo.
    fn is_past(&self, node: Node<'_>, comment: Node<'_>) -> bool {
        let start = comment.start_byte();
        if start < node.end_byte() {
            return false;
        }
        self.has_token_between(node.end_byte(), start)
            || (self.is_own_line(comment)
                && indentation_at(self.source, node.start_byte()).unwrap_or(0)
                    >= self.comment_indentation_after(node, comment))
    }

    fn capture(&mut self, comment: Suppression<'tree>, placement: &Placement<'tree>) {
        self.next = self.next.saturating_add(1);
        if self.is_ignored(comment, placement) {
            self.reports.push(
                Report::at(PythonRule::RUF028, comment.node, "").with_fix(Some(
                    delete_comment(comment.node.start_byte(), comment.node.end_byte(), self.source)
                        .marked_unsafe(),
                )),
            );
        }
    }

    fn is_ignored(&self, comment: Suppression<'tree>, placement: &Placement<'tree>) -> bool {
        let own_line = self.is_own_line(comment.node);
        if placement
            .enclosing
            .is_some_and(|enclosing| !is_statement(enclosing))
        {
            return true;
        }
        if comment.kind == Kind::Skip {
            return own_line;
        }
        if own_line
            && placement
                .enclosing
                .is_some_and(|enclosing| is_after_decorator(enclosing, comment.node))
        {
            return true;
        }
        if comment.kind == Kind::On {
            return !own_line;
        }
        own_line && self.is_above_alternate_body(comment.node, placement)
    }

    // Un `# fmt: off` justo arriba de un `else`, `elif`, `except` o `finally`, con otra indentación que
    // la sentencia anterior.
    fn is_above_alternate_body(&self, comment: Node<'tree>, placement: &Placement<'tree>) -> bool {
        let (Some(enclosing), Some(preceding), Some(following)) =
            (placement.enclosing, placement.preceding, placement.following)
        else {
            return false;
        };
        is_first_in_alternate_body(following, enclosing)
            && self.comment_indentation_after(preceding, comment)
                != indentation_at(self.source, preceding.start_byte()).unwrap_or(0)
    }

    fn is_own_line(&self, comment: Node<'_>) -> bool {
        indentation_at(self.source, comment.start_byte()).is_some()
    }

    // `comment_indentation_after`: la menor indentación de los comentarios propios entre la línea
    // siguiente al nodo y el comentario.
    fn comment_indentation_after(&self, node: Node<'_>, comment: Node<'_>) -> usize {
        let from = line_end(self.source, node.end_byte());
        self.comments
            .iter()
            .filter(|other| other.start_byte() >= from && other.end_byte() <= comment.end_byte())
            .filter_map(|other| indentation_at(self.source, other.start_byte()))
            .min()
            .unwrap_or(0)
    }

    // Si entre los dos puntos hay algo más que espacios, saltos de línea, `\` y comentarios.
    fn has_token_between(&self, start: usize, end: usize) -> bool {
        let between = self.source.get(start..end).unwrap_or_default();
        let mut index = 0;
        while let Some(byte) = between.get(index) {
            if *byte == HASH_BYTE {
                index = between
                    .iter()
                    .skip(index)
                    .position(|candidate| *candidate == NEWLINE)
                    .map_or(between.len(), |offset| index.saturating_add(offset));
                continue;
            }
            if !TRIVIA.contains(byte) && *byte != BACKSLASH {
                return true;
            }
            index = index.saturating_add(1);
        }
        false
    }
}

// Un `# fmt: off` o `on` entre el primer decorador y el nombre de la función o clase.
fn is_after_decorator(enclosing: Node<'_>, comment: Node<'_>) -> bool {
    if enclosing.kind() != DECORATED {
        return false;
    }
    let first_decorator = named(enclosing)
        .into_iter()
        .find(|child| child.kind() == DECORATOR);
    let name = enclosing
        .child_by_field_name(DEFINITION_FIELD)
        .and_then(|definition| definition.child_by_field_name(NAME_FIELD));
    match (first_decorator, name) {
        (Some(decorator), Some(name)) => {
            (decorator.end_byte()..name.start_byte()).contains(&comment.start_byte())
        }
        _ => false,
    }
}

// `indentation_at_offset`: lo que hay antes en la línea, si son sólo espacios.
fn indentation_at(source: &[u8], offset: usize) -> Option<usize> {
    let before = source.get(..offset).unwrap_or_default();
    let line_start = before
        .iter()
        .rposition(|byte| *byte == NEWLINE)
        .map_or(0, |newline| newline.saturating_add(1));
    let indentation = before.get(line_start..).unwrap_or_default();
    indentation
        .iter()
        .all(|byte| INDENTATION.contains(byte))
        .then_some(indentation.len())
}

fn line_end(source: &[u8], offset: usize) -> usize {
    source
        .iter()
        .skip(offset)
        .position(|byte| *byte == NEWLINE)
        .map_or(source.len(), |position| {
            offset.saturating_add(position).saturating_add(1)
        })
}
