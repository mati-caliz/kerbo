use std::ops::Range;

use tree_sitter::{Node, Parser, Tree};

use super::arguments::{heredoc_body, name, raw};
use crate::language::Language;
use crate::syntax::{descendants, named_children};

const CALL: &str = "call";
const EACH: &str = "each";
const FREEZE: &str = "freeze";
const ARRAY: &str = "array";
const ARRAY_KINDS: &[&str] = &[ARRAY, "string_array", "symbol_array"];
const SYMBOL_KINDS: &[&str] = &["simple_symbol", "delimited_symbol", "bare_symbol"];
const INTEGER: &str = "integer";
const COMMENT: &str = "comment";
const ASSIGNMENT: &str = "assignment";
const CONSTANT: &str = "constant";
const IDENTIFIER: &str = "identifier";
const INTERPOLATION: &str = "interpolation";
const BLOCK_PARAMETERS: &str = "block_parameters";
const HEREDOC_BEGINNING: &str = "heredoc_beginning";
const BLOCK_CLOSINGS: &[&str] = &["end", "}"];
const FLOW_CHANGES: &[&str] = &["next", "break", "redo", "retry", "return"];
const UNSAFE_CHARACTERS: &[char] = &['"', '\\', '#', '\n'];
const REPETITION_SEPARATOR: &str = "\n";
const NEWLINE: u8 = b'\n';
const MAXIMUM_PASSES: usize = 4;
const MAXIMUM_ELEMENTS: usize = 64;

pub struct Expansion {
    pub source: String,
    pub rows: Vec<usize>,
}

struct Value {
    literal: String,
    text: String,
}

struct Occurrence {
    range: Range<usize>,
    parameter: usize,
    interpolated: bool,
}

struct Unrolled {
    range: Range<usize>,
    body: Range<usize>,
    iterations: Vec<Vec<Value>>,
    occurrences: Vec<Occurrence>,
}

pub fn parse(source: &str) -> Option<Tree> {
    let mut parser = Parser::new();
    parser.set_language(&Language::Rails.grammar()?).ok()?;
    parser.parse(source, None)
}

pub fn expand(root: Node<'_>, source: &[u8]) -> Option<Expansion> {
    let mut current = pass(root, source)?;
    for _ in 1..MAXIMUM_PASSES {
        let Some(tree) = parse(&current.source) else {
            break;
        };
        let Some(next) = pass(tree.root_node(), current.source.as_bytes()) else {
            break;
        };
        let rows = next
            .rows
            .iter()
            .map(|row| current.rows.get(*row).copied().unwrap_or(*row))
            .collect();
        current = Expansion {
            source: next.source,
            rows,
        };
    }
    Some(current)
}

fn pass(root: Node<'_>, source: &[u8]) -> Option<Expansion> {
    let mut candidates: Vec<Unrolled> = descendants(root)
        .into_iter()
        .filter_map(|node| unrolled(node, root, source))
        .collect();
    candidates.sort_by_key(|candidate| candidate.range.start);
    let mut outermost: Vec<Unrolled> = Vec::new();
    for candidate in candidates {
        if outermost
            .last()
            .is_none_or(|last| candidate.range.start >= last.range.end)
        {
            outermost.push(candidate);
        }
    }
    if outermost.is_empty() {
        return None;
    }
    let mut emitter = Emitter::new(source);
    let mut position = 0;
    for unrolled in &outermost {
        emitter.original(position..unrolled.range.start);
        emitter.repeat(unrolled);
        position = unrolled.range.end;
    }
    emitter.original(position..source.len());
    Some(emitter.finish())
}

fn unrolled(node: Node<'_>, root: Node<'_>, source: &[u8]) -> Option<Unrolled> {
    if node.kind() != CALL || raw(node.child_by_field_name("method")?, source) != EACH {
        return None;
    }
    let elements = elements(node.child_by_field_name("receiver")?, root, source)?;
    let block = node.child_by_field_name("block")?;
    let parameters = block.child_by_field_name("parameters")?;
    let names = named_children(parameters)
        .into_iter()
        .map(|parameter| (parameter.kind() == IDENTIFIER).then(|| raw(parameter, source)))
        .collect::<Option<Vec<_>>>()?;
    let closing = block.child(block.child_count().checked_sub(1)?)?;
    if names.is_empty() || !BLOCK_CLOSINGS.contains(&closing.kind()) {
        return None;
    }
    let body = parameters.end_byte()..closing.start_byte();
    let iterations = elements
        .into_iter()
        .map(|element| bindings(element, names.len(), source))
        .collect::<Option<Vec<_>>>()?;
    let occurrences = occurrences(root, &body, &names, source)?;
    Some(Unrolled {
        range: node.start_byte()..node.end_byte(),
        body,
        iterations,
        occurrences,
    })
}

fn elements<'tree>(receiver: Node<'tree>, root: Node<'tree>, source: &[u8]) -> Option<Vec<Node<'tree>>> {
    let list = match receiver.kind() {
        CONSTANT => constant_value(root, &raw(receiver, source), source)?,
        _ => without_freeze(receiver, source),
    };
    if !ARRAY_KINDS.contains(&list.kind()) {
        return None;
    }
    let elements: Vec<_> = named_children(list)
        .into_iter()
        .filter(|element| element.kind() != COMMENT)
        .collect();
    (elements.len() <= MAXIMUM_ELEMENTS).then_some(elements)
}

fn without_freeze<'tree>(node: Node<'tree>, source: &[u8]) -> Node<'tree> {
    let frozen = node.kind() == CALL
        && node
            .child_by_field_name("method")
            .is_some_and(|method| raw(method, source) == FREEZE);
    frozen
        .then(|| node.child_by_field_name("receiver"))
        .flatten()
        .unwrap_or(node)
}

fn constant_value<'tree>(root: Node<'tree>, constant: &str, source: &[u8]) -> Option<Node<'tree>> {
    descendants(root)
        .into_iter()
        .filter(|node| node.kind() == ASSIGNMENT)
        .find(|assignment| {
            assignment
                .child_by_field_name("left")
                .is_some_and(|left| left.kind() == CONSTANT && raw(left, source) == constant)
        })
        .and_then(|assignment| assignment.child_by_field_name("right"))
        .map(|value| without_freeze(value, source))
}

fn bindings(element: Node<'_>, parameters: usize, source: &[u8]) -> Option<Vec<Value>> {
    if parameters == 1 {
        return Some(vec![value(element, source)?]);
    }
    let parts: Vec<_> = (element.kind() == ARRAY)
        .then(|| named_children(element))?
        .into_iter()
        .filter(|part| part.kind() != COMMENT)
        .collect();
    if parts.len() != parameters {
        return None;
    }
    parts.into_iter().map(|part| value(part, source)).collect()
}

fn value(node: Node<'_>, source: &[u8]) -> Option<Value> {
    let text = if node.kind() == INTEGER {
        raw(node, source)
    } else {
        name(node, source)?
    };
    if text.contains(UNSAFE_CHARACTERS) {
        return None;
    }
    let literal = match node.kind() {
        INTEGER => text.clone(),
        kind if SYMBOL_KINDS.contains(&kind) => format!(":\"{text}\""),
        _ => format!("\"{text}\""),
    };
    Some(Value { literal, text })
}

enum Visit {
    Bail,
    Found(Occurrence),
    Ignore,
}

fn occurrences(
    root: Node<'_>,
    body: &Range<usize>,
    names: &[String],
    source: &[u8],
) -> Option<Vec<Occurrence>> {
    let mut found = Vec::new();
    let inside = descendants(root)
        .into_iter()
        .filter(|node| node.start_byte() >= body.start && node.end_byte() <= body.end);
    for node in inside {
        match visit(node, body, names, source) {
            Visit::Bail => return None,
            Visit::Found(occurrence) => found.push(occurrence),
            Visit::Ignore => {}
        }
    }
    found.sort_by_key(|occurrence| occurrence.range.start);
    Some(found)
}

fn visit(node: Node<'_>, body: &Range<usize>, names: &[String], source: &[u8]) -> Visit {
    let parameter = |candidate: Node<'_>| names.iter().position(|name| *name == raw(candidate, source));
    let found = |position: usize, interpolated: bool| {
        Visit::Found(Occurrence {
            range: node.start_byte()..node.end_byte(),
            parameter: position,
            interpolated,
        })
    };
    match node.kind() {
        kind if FLOW_CHANGES.contains(&kind) => Visit::Bail,
        BLOCK_PARAMETERS
            if named_children(node)
                .into_iter()
                .any(|inner| parameter(inner).is_some()) =>
        {
            Visit::Bail
        }
        HEREDOC_BEGINNING if heredoc_body(node).is_none_or(|heredoc| heredoc.end_byte() > body.end) => {
            Visit::Bail
        }
        INTERPOLATION => match named_children(node).as_slice() {
            [inner] if inner.kind() == IDENTIFIER => {
                parameter(*inner).map_or(Visit::Ignore, |position| found(position, true))
            }
            _ => Visit::Ignore,
        },
        IDENTIFIER if !is_interpolated_alone(node) && !is_method_name(node) => match parameter(node) {
            Some(_) if is_assigned(node) => Visit::Bail,
            Some(position) => found(position, false),
            None => Visit::Ignore,
        },
        _ => Visit::Ignore,
    }
}

fn is_interpolated_alone(node: Node<'_>) -> bool {
    node.parent()
        .is_some_and(|parent| parent.kind() == INTERPOLATION && parent.named_child_count() == 1)
}

fn is_method_name(node: Node<'_>) -> bool {
    node.parent().is_some_and(|parent| {
        parent.kind() == CALL
            && parent.child_by_field_name("receiver").is_some()
            && parent.child_by_field_name("method") == Some(node)
    })
}

fn is_assigned(node: Node<'_>) -> bool {
    node.parent().is_some_and(|parent| {
        parent.kind().ends_with(ASSIGNMENT) && parent.child_by_field_name("left") == Some(node)
    })
}

struct Emitter<'source> {
    source: &'source [u8],
    line_starts: Vec<usize>,
    text: String,
    rows: Vec<usize>,
}

impl<'source> Emitter<'source> {
    fn new(source: &'source [u8]) -> Self {
        let line_starts = std::iter::once(0)
            .chain(
                source
                    .iter()
                    .enumerate()
                    .filter(|(_, byte)| **byte == NEWLINE)
                    .map(|(offset, _)| offset + 1),
            )
            .collect();
        Self {
            source,
            line_starts,
            text: String::new(),
            rows: vec![0],
        }
    }

    fn row_of(&self, offset: usize) -> usize {
        self.line_starts
            .partition_point(|start| *start <= offset)
            .saturating_sub(1)
    }

    fn original(&mut self, range: Range<usize>) {
        let mut row = self.row_of(range.start);
        let piece = String::from_utf8_lossy(self.source.get(range).unwrap_or_default()).into_owned();
        for _ in piece.matches(char::from(NEWLINE)) {
            row += 1;
            self.rows.push(row);
        }
        self.text.push_str(&piece);
    }

    fn inserted(&mut self, text: &str, row: usize) {
        for _ in text.matches(char::from(NEWLINE)) {
            self.rows.push(row);
        }
        self.text.push_str(text);
    }

    fn repeat(&mut self, unrolled: &Unrolled) {
        let row = self.row_of(unrolled.body.start);
        for (index, values) in unrolled.iterations.iter().enumerate() {
            if index > 0 {
                self.inserted(REPETITION_SEPARATOR, row);
            }
            let mut position = unrolled.body.start;
            for occurrence in &unrolled.occurrences {
                self.original(position..occurrence.range.start);
                self.text.push_str(replacement(values, occurrence));
                position = occurrence.range.end;
            }
            self.original(position..unrolled.body.end);
        }
    }

    fn finish(self) -> Expansion {
        Expansion {
            source: self.text,
            rows: self.rows,
        }
    }
}

fn replacement<'value>(values: &'value [Value], occurrence: &Occurrence) -> &'value str {
    values.get(occurrence.parameter).map_or("", |value| {
        if occurrence.interpolated {
            value.text.as_str()
        } else {
            value.literal.as_str()
        }
    })
}
