pub mod embedded;
pub mod typing;

use crate::lint::python::semantic::annotations::embedded::EmbeddedAnnotations;
use crate::lint::python::semantic::expressions::Expressions;
use tree_sitter::{Node, Point};

use super::pep::{is_pep646_unpack, is_typing_only, is_typing_only_annotation, pep604_rule};
use super::qualified::{is_typing, qualified_name};
use super::{Mode, Pending, Visitor};
use crate::fix::Fix;
use crate::lint::python::Report;
use crate::lint::python::ast::{
    COMMENT, FUNCTION, PARENTHESIZED, STRING, SUBSCRIPT_FIELD, TUPLE, VALUE_FIELD, fields, is_bytes,
    is_f_string, named, string_prefix, text,
};
use crate::lint::python::escapes::decoded;
use crate::rules::PythonRule;
use crate::syntax::{SnippetLanguage, parse_snippet};
use embedded::{Embedded, Placement, wrap};

const PARAMETERS: &str = "parameters";
const LITERAL: &str = "Literal";
const ANNOTATED: &str = "Annotated";
const TYPE_ALIAS: &str = "TypeAlias";
const TRIPLE_QUOTES: [&str; 2] = ["\"\"\"", "'''"];
const QUOTES: [&str; 2] = ["\"", "'"];
const RAW: [char; 2] = ['r', 'R'];
const TYPE_NODE: &str = "type";
const STARRED: [&str; 2] = ["list_splat", "parenthesized_list_splat"];

#[derive(Debug, Clone)]
pub struct Content {
    pub value: String,
    pub simple: bool,
    pub origin: Point,
}

pub trait Annotations<'tree> {
    fn annotation(&mut self, annotation: Node<'tree>);
    fn is_type_alias(&self, annotation: Node<'_>) -> bool;
    fn subscript(&mut self, subscript: Node<'tree>, mode: Mode);
    fn generic_type(&mut self, generic: Node<'tree>, mode: Mode);
    fn slices(&mut self, value: Option<Node<'tree>>, slices: Vec<Node<'tree>>, mode: Mode);
    fn pep604_operator(&mut self, subscript: Node<'tree>, value: Option<Node<'tree>>, slices: &[Node<'tree>]);
    fn string_annotation(&mut self, pending: Pending<'tree>);
    fn content(&self, part: Node<'_>) -> Content;
    fn parse_annotation(&mut self, value: &str, placement: Placement, string: Node<'_>);
}

impl<'tree> Annotations<'tree> for Visitor<'tree> {
    fn annotation(&mut self, annotation: Node<'tree>) {
        let annotation = inner_type(annotation);
        if (self.boundaries.future_annotations() || self.file.stub) && !self.model.forward_reference {
            self.deferred.annotations.push(self.snapshot(annotation));
        } else {
            self.expression(annotation, Mode::TypeDefinition);
        }
    }

    fn is_type_alias(&self, annotation: Node<'_>) -> bool {
        qualified_name(&self.model, inner_type(annotation), self.source)
            .is_some_and(|name| is_typing(&name, TYPE_ALIAS))
    }

    fn subscript(&mut self, subscript: Node<'tree>, mode: Mode) {
        let value = subscript.child_by_field_name(VALUE_FIELD);
        if let Some(value) = value {
            self.expression(value, mode);
        }
        self.pep604_operator(subscript, value, &fields(subscript, SUBSCRIPT_FIELD));
        self.slices(value, fields(subscript, SUBSCRIPT_FIELD), mode);
    }

    fn generic_type(&mut self, generic: Node<'tree>, mode: Mode) {
        let parts = named(generic);
        let Some((value, rest)) = parts.split_first() else {
            return;
        };
        self.expression(*value, mode);
        let slices: Vec<Node<'tree>> = rest.iter().flat_map(|parameters| named(*parameters)).collect();
        self.pep604_operator(generic, Some(*value), &slices);
        self.slices(Some(*value), slices, mode);
    }

    // UP045 y UP007: `Optional[X]` y `Union[X, Y]`, que en 3.10 se escriben con `|`; UP044: `Unpack[X]`.
    fn pep604_operator(
        &mut self,
        subscript: Node<'tree>,
        value: Option<Node<'tree>>,
        slices: &[Node<'tree>],
    ) {
        let Some(name) = value.and_then(|value| qualified_name(&self.model, value, self.source)) else {
            return;
        };
        let typing_context =
            self.file.stub || is_typing_only(subscript, self.boundaries.future_annotations(), self.source);
        let qualify = |node: Node<'_>| qualified_name(&self.model, node, self.source);
        if let Some(rule) = pep604_rule(&name, slices, &qualify, typing_context) {
            self.reports.push(Report::at(rule, subscript, ""));
        }
        if is_pep646_unpack(&name, subscript, slices) {
            self.reports.push(Report::at(PythonRule::UP044, subscript, ""));
        }
    }

    fn slices(&mut self, value: Option<Node<'tree>>, slices: Vec<Node<'tree>>, mode: Mode) {
        let name = value.and_then(|value| qualified_name(&self.model, value, self.source));
        let modes = slice_modes(name.as_deref(), mode, slices.len());
        for (slice, slice_mode) in slices.into_iter().zip(modes) {
            self.expression(slice, slice_mode);
        }
    }

    fn string_annotation(&mut self, pending: Pending<'tree>) {
        self.restore(pending);
        let string = pending.node;
        let parts = if string.kind() == STRING {
            vec![string]
        } else {
            named(string)
        };
        if parts
            .iter()
            .any(|part| is_f_string(*part, self.source) || is_bytes(*part, self.source))
        {
            return;
        }
        let contents: Vec<Content> = parts.iter().map(|part| self.content(*part)).collect();
        let simple = matches!(contents.as_slice(), [only] if only.simple);
        let origin = contents
            .first()
            .map_or_else(|| string.start_position(), |first| first.origin);
        let value: String = contents.into_iter().map(|content| content.value).collect();
        let placement = Placement {
            simple,
            origin,
            fallback: string.start_position(),
        };
        self.parse_annotation(&value, placement, string);
    }

    fn content(&self, part: Node<'_>) -> Content {
        let prefix = string_prefix(part, self.source);
        let literal = text(part, self.source).get(prefix.len()..).unwrap_or_default();
        let quote = TRIPLE_QUOTES
            .iter()
            .chain(QUOTES.iter())
            .find(|quote| literal.starts_with(**quote))
            .map_or(0, |quote| quote.len());
        let raw = literal
            .get(quote..literal.len().saturating_sub(quote))
            .unwrap_or_default();
        let value = if prefix.contains(RAW) {
            raw.to_owned()
        } else {
            decoded(raw)
        };
        let start = part.start_position();
        Content {
            simple: value == raw,
            value,
            origin: Point {
                row: start.row,
                column: start.column.saturating_add(prefix.len()).saturating_add(quote),
            },
        }
    }

    fn parse_annotation(&mut self, value: &str, placement: Placement, string: Node<'_>) {
        let wrapped = wrap(value);
        let Some(tree) = parse_snippet(SnippetLanguage::Python, &wrapped) else {
            return;
        };
        let root = tree.root_node();
        let expression = named(root)
            .first()
            .and_then(|statement| named(*statement).first().copied())
            .map(|wrapper| {
                if wrapper.kind() == PARENTHESIZED {
                    named(wrapper).first().copied().unwrap_or(wrapper)
                } else {
                    wrapper
                }
            });
        let valid = !root.has_error() && named(root).len() == 1 && !value.trim().is_empty();
        let Some(expression) = expression.filter(|expression| valid && !is_starred(*expression)) else {
            self.reports
                .push(Report::at(PythonRule::F722, string, value.trim()));
            return;
        };
        if self.file.stub {
            self.reports.push(Report::at(PythonRule::PYI020, string, ""));
        }
        let typing_only =
            is_typing_only_annotation(string, self.boundaries.future_annotations() || self.file.stub);
        if typing_only {
            let fix = unquoted_annotation(value, root, string, self.source);
            self.reports
                .push(Report::at(PythonRule::UP037, string, "").with_fix(Some(fix)));
        }
        let context = Embedded {
            source: wrapped.as_bytes(),
            placement: placement.wrapped(),
            literal: false,
            locals: Vec::new(),
            typing_only,
        };
        self.embedded_loads(expression, &context);
    }
}

fn inner_type(annotation: Node<'_>) -> Node<'_> {
    if annotation.kind() == TYPE_NODE {
        named(annotation).first().copied().unwrap_or(annotation)
    } else {
        annotation
    }
}

fn slice_modes(name: Option<&str>, mode: Mode, count: usize) -> Vec<Mode> {
    let first_and_rest = |first: Mode, rest: Mode| {
        std::iter::once(first)
            .chain(std::iter::repeat(rest))
            .take(count)
            .collect()
    };
    match name {
        Some(name) if is_typing(name, LITERAL) => vec![Mode::Literal; count],
        Some(_) if mode == Mode::Literal => vec![mode; count],
        Some(name) if is_typing(name, ANNOTATED) => first_and_rest(Mode::TypeDefinition, Mode::Value),
        Some(name) if typing::is_generic(name) => vec![Mode::TypeDefinition; count],
        _ => vec![mode; count],
    }
}

fn is_starred(expression: Node<'_>) -> bool {
    STARRED.contains(&expression.kind())
        || (expression.kind() == TUPLE
            && named(expression)
                .into_iter()
                .any(|element| STARRED.contains(&element.kind())))
}

// UP037 deja la anotación sin comillas; si ocupa varias líneas va entre paréntesis (salvo en un
// parámetro), y si termina en un comentario el paréntesis cierra en la línea siguiente.
fn unquoted_annotation(value: &str, parsed: Node<'_>, string: Node<'_>, source: &[u8]) -> Fix {
    let last_content = value.trim_end().len().saturating_add(1);
    let ends_in_comment = crate::syntax::descendants(parsed)
        .iter()
        .any(|node| node.kind() == COMMENT && node.end_byte() == last_content);
    let multiline = value.contains(['\n', '\r']);
    let in_parameter = std::iter::successors(string.parent(), Node::parent).any(|ancestor| {
        ancestor.kind() == PARAMETERS && ancestor.parent().is_some_and(|owner| owner.kind() == FUNCTION)
    });
    let content = match (multiline, ends_in_comment) {
        (_, false) if in_parameter => value.to_owned(),
        (false, false) => value.to_owned(),
        (true, false) => format!("({value})"),
        (_, true) => format!("({value}\n)"),
    };
    Fix::replace(string, &content, source)
}
