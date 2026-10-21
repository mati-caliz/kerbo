use tree_sitter::{Node, Point};

use super::super::Visitor;
use super::super::qualified::{is_typing, qualified_name};
use super::{ANNOTATED, LITERAL};
use crate::lint::python::ast::{
    ATTRIBUTE, CONCATENATED_STRING, IDENTIFIER, KEYWORD_ARGUMENT, LAMBDA, OBJECT_FIELD, STRING, SUBSCRIPT,
    SUBSCRIPT_FIELD, VALUE_FIELD, fields, is_bytes, is_f_string, named, string_prefix, text,
};
use crate::lint::python::escapes::decoded;
use crate::syntax::{SnippetLanguage, parse_snippet};

pub const OPENING: &str = "(";
const CLOSING: &str = ")";
const QUOTES: [&str; 2] = ["\"", "'"];
const PARAMETERS_FIELD: &str = "parameters";
const BODY_FIELD: &str = "body";

#[derive(Debug, Clone, Copy)]
pub struct Placement {
    pub simple: bool,
    pub origin: Point,
    pub fallback: Point,
}

impl Placement {
    pub const fn map(self, point: Point) -> Point {
        if !self.simple {
            return self.fallback;
        }
        if point.row == 0 {
            Point {
                row: self.origin.row,
                column: self.origin.column.saturating_add(point.column),
            }
        } else {
            Point {
                row: self.origin.row.saturating_add(point.row),
                column: point.column,
            }
        }
    }

    pub const fn wrapped(self) -> Self {
        Self {
            origin: Point {
                row: self.origin.row,
                column: self.origin.column.saturating_sub(OPENING.len()),
            },
            ..self
        }
    }
}

#[derive(Debug, Clone)]
pub struct Embedded<'source> {
    pub source: &'source [u8],
    pub placement: Placement,
    pub literal: bool,
    pub locals: Vec<String>,
}

impl Embedded<'_> {
    fn literal(&self, literal: bool) -> Self {
        Self {
            literal,
            ..self.clone()
        }
    }
}

pub fn wrap(value: &str) -> String {
    format!("{OPENING}{value}{CLOSING}")
}

pub trait EmbeddedAnnotations {
    fn embedded_loads(&mut self, node: Node<'_>, context: &Embedded<'_>);
    fn embedded_subscript(&mut self, node: Node<'_>, context: &Embedded<'_>);
    fn embedded_lambda(&mut self, lambda: Node<'_>, context: &Embedded<'_>);
    fn nested_annotation(&mut self, string: Node<'_>, context: &Embedded<'_>);
}

impl EmbeddedAnnotations for Visitor<'_> {
    fn embedded_loads(&mut self, node: Node<'_>, context: &Embedded<'_>) {
        match node.kind() {
            IDENTIFIER => {
                let name = text(node, context.source);
                if !context.locals.iter().any(|local| local == name) {
                    self.model
                        .resolve_load(name, context.placement.map(node.start_position()));
                }
            }
            ATTRIBUTE => {
                if let Some(object) = node.child_by_field_name(OBJECT_FIELD) {
                    self.embedded_loads(object, context);
                }
            }
            SUBSCRIPT => self.embedded_subscript(node, context),
            STRING | CONCATENATED_STRING if !context.literal => self.nested_annotation(node, context),
            STRING | CONCATENATED_STRING => {}
            KEYWORD_ARGUMENT => {
                if let Some(value) = node.child_by_field_name(VALUE_FIELD) {
                    self.embedded_loads(value, context);
                }
            }
            LAMBDA => self.embedded_lambda(node, context),
            _ => {
                for child in named(node) {
                    self.embedded_loads(child, context);
                }
            }
        }
    }

    fn embedded_subscript(&mut self, node: Node<'_>, context: &Embedded<'_>) {
        let value = node.child_by_field_name(VALUE_FIELD);
        if let Some(value) = value {
            self.embedded_loads(value, context);
        }
        let name = value.and_then(|value| qualified_name(&self.model, value, context.source));
        let is_literal = context.literal || name.as_deref().is_some_and(|name| is_typing(name, LITERAL));
        let is_annotated = name.as_deref().is_some_and(|name| is_typing(name, ANNOTATED));
        for (index, slice) in fields(node, SUBSCRIPT_FIELD).into_iter().enumerate() {
            let inner = context.literal(is_literal || (is_annotated && index > 0));
            self.embedded_loads(slice, &inner);
        }
    }

    fn embedded_lambda(&mut self, lambda: Node<'_>, context: &Embedded<'_>) {
        let mut inner = context.clone();
        let parameters = lambda
            .child_by_field_name(PARAMETERS_FIELD)
            .map(named)
            .unwrap_or_default();
        for parameter in parameters {
            let name = std::iter::successors(Some(parameter), |node| named(*node).first().copied())
                .find(|node| node.kind() == IDENTIFIER);
            inner
                .locals
                .extend(name.map(|name| text(name, context.source).to_owned()));
        }
        if let Some(body) = lambda.child_by_field_name(BODY_FIELD) {
            self.embedded_loads(body, &inner);
        }
    }

    fn nested_annotation(&mut self, string: Node<'_>, context: &Embedded<'_>) {
        let source = context.source;
        let parts = if string.kind() == STRING {
            vec![string]
        } else {
            named(string)
        };
        if parts
            .iter()
            .any(|part| is_f_string(*part, source) || is_bytes(*part, source))
        {
            return;
        }
        let literal = text(string, source);
        let prefix = string_prefix(string, source);
        let body = literal.get(prefix.len()..).unwrap_or_default();
        let quote = QUOTES
            .iter()
            .find(|quote| body.starts_with(**quote))
            .map_or(0, |quote| quote.len());
        let raw = body
            .get(quote..body.len().saturating_sub(quote))
            .unwrap_or_default();
        let value = decoded(raw);
        let start = string.start_position();
        let placement = Placement {
            simple: context.placement.simple && value == raw && parts.len() == 1,
            origin: context.placement.map(Point {
                row: start.row,
                column: start.column.saturating_add(prefix.len()).saturating_add(quote),
            }),
            fallback: context.placement.map(start),
        };
        let wrapped = wrap(&value);
        if let Some(tree) = parse_snippet(SnippetLanguage::Python, &wrapped) {
            let nested = Embedded {
                source: wrapped.as_bytes(),
                placement: placement.wrapped(),
                literal: false,
                locals: context.locals.clone(),
            };
            self.embedded_loads(tree.root_node(), &nested);
        }
    }
}
