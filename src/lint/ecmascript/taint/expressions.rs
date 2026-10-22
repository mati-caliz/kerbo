use tree_sitter::Node;

use super::calls::{call, sink_at};
use super::statements::Interpreter;
use super::tables;
use crate::lint::taint::folding::{fold_binary, fold_unary, truth};
use crate::lint::taint::{Atom, Flow, Literal, SinkKind, State, Value, element};
use crate::syntax::named_children;

const IDENTIFIER: &str = "identifier";
const STRING: &str = "string";
const STRING_FRAGMENT: &str = "string_fragment";
const TEMPLATE: &str = "template_string";
const SUBSTITUTION: &str = "template_substitution";
const NUMBER: &str = "number";
const TRUE: &str = "true";
const FALSE: &str = "false";
const MEMBER: &str = "member_expression";
const SUBSCRIPT: &str = "subscript_expression";
const CALL: &str = "call_expression";
const NEW: &str = "new_expression";
const ASSIGNMENT: &str = "assignment_expression";
const AUGMENTED: &str = "augmented_assignment_expression";
const BINARY: &str = "binary_expression";
const UNARY: &str = "unary_expression";
const TERNARY: &str = "ternary_expression";
const ARRAY: &str = "array";
const OBJECT: &str = "object";
const PAIR: &str = "pair";
const SHORTHAND: &str = "shorthand_property_identifier";
const OBJECT_PATTERN: &str = "object_pattern";
const ARRAY_PATTERN: &str = "array_pattern";
const PAIR_PATTERN: &str = "pair_pattern";
const SHORTHAND_PATTERN: &str = "shorthand_property_identifier_pattern";
const ASSIGNMENT_PATTERN: &str = "assignment_pattern";
const JSX_ATTRIBUTE: &str = "jsx_attribute";
const WRAPPERS: &[&str] = &[
    "parenthesized_expression",
    "await_expression",
    "as_expression",
    "satisfies_expression",
    "non_null_expression",
    "type_assertion",
];
pub(super) const CLOSURES: &[&str] = &["arrow_function", "function_expression", "function"];
const OPAQUE: &[&str] = &["class", "regex", "null", "undefined"];
const OBJECT_FIELD: &str = "object";
const PROPERTY_FIELD: &str = "property";
const INDEX_FIELD: &str = "index";
const LEFT_FIELD: &str = "left";
const RIGHT_FIELD: &str = "right";
const OPERATOR_FIELD: &str = "operator";
const ARGUMENT_FIELD: &str = "argument";
const CONDITION_FIELD: &str = "condition";
const CONSEQUENCE_FIELD: &str = "consequence";
const ALTERNATIVE_FIELD: &str = "alternative";
const KEY_FIELD: &str = "key";
const VALUE_FIELD: &str = "value";
const PLUS: &str = "+";
const KEEPS_OPERANDS: &[&str] = &["+", "||", "??", "&&"];
const DANGEROUS_HTML: &str = "dangerouslySetInnerHTML";
const HTML_KEY: &str = "__html";
const DIGIT_SEPARATOR: char = '_';

// El valor de un literal: un texto sin escapes, un entero o un booleano.
pub fn literal_of(node: Node<'_>, source: &[u8]) -> Option<Literal> {
    let text = |part: Node<'_>| part.utf8_text(source).unwrap_or_default();
    match node.kind() {
        STRING => Some(Literal::Text(
            named_children(node)
                .into_iter()
                .filter(|part| part.kind() == STRING_FRAGMENT)
                .map(text)
                .collect(),
        )),
        NUMBER => text(node)
            .replace(DIGIT_SEPARATOR, "")
            .parse()
            .ok()
            .map(Literal::Integer),
        TRUE => Some(Literal::Boolean(true)),
        FALSE => Some(Literal::Boolean(false)),
        _ => None,
    }
}

impl Interpreter<'_, '_> {
    pub(super) fn expression(&mut self, node: Node<'_>, state: &mut State) -> Value {
        if let Some(literal) = literal_of(node, self.source) {
            return Value::Constant(literal);
        }
        let field = |name: &str| node.child_by_field_name(name);
        match node.kind() {
            IDENTIFIER => state.get(self.text(node)).cloned().unwrap_or_else(Value::clean),
            TEMPLATE => self.template(node, state),
            MEMBER => self.member(node, state),
            SUBSCRIPT => {
                let collection =
                    field(OBJECT_FIELD).map_or_else(Value::clean, |inner| self.expression(inner, state));
                let index =
                    field(INDEX_FIELD).map_or_else(Value::clean, |inner| self.expression(inner, state));
                element(&collection, &index)
            }
            CALL | NEW => call(self, node, state),
            ASSIGNMENT | AUGMENTED => self.assignment(node, state),
            BINARY => self.binary(node, state),
            UNARY => {
                let operand =
                    field(ARGUMENT_FIELD).map_or_else(Value::clean, |inner| self.expression(inner, state));
                let operator = field(OPERATOR_FIELD)
                    .map(|written| self.text(written))
                    .unwrap_or_default();
                operand
                    .literal()
                    .and_then(|literal| fold_unary(operator, literal))
                    .map_or_else(Value::clean, Value::Constant)
            }
            TERNARY => {
                let condition =
                    field(CONDITION_FIELD).map_or_else(Value::clean, |inner| self.expression(inner, state));
                let mut branch = |name: &str, current: &mut State| {
                    field(name).map_or_else(Value::clean, |inner| self.expression(inner, current))
                };
                match truth(condition.literal()) {
                    Some(true) => branch(CONSEQUENCE_FIELD, state),
                    Some(false) => branch(ALTERNATIVE_FIELD, state),
                    None => {
                        let taken = branch(CONSEQUENCE_FIELD, state);
                        taken.join(branch(ALTERNATIVE_FIELD, state))
                    }
                }
            }
            ARRAY => Value::List(
                named_children(node)
                    .into_iter()
                    .map(|item| self.expression(item, state))
                    .collect(),
            ),
            OBJECT => self.object(node, state),
            JSX_ATTRIBUTE => self.jsx_attribute(node, state),
            kind if WRAPPERS.contains(&kind) => named_children(node)
                .first()
                .map_or_else(Value::clean, |inner| self.expression(*inner, state)),
            kind if CLOSURES.contains(&kind) => self.closure(node, state, &Value::clean()),
            kind if OPAQUE.contains(&kind) => Value::clean(),
            _ => Value::Data(
                named_children(node)
                    .into_iter()
                    .fold(Flow::default(), |flow, child| {
                        flow.union(&self.expression(child, state).flow())
                    }),
            ),
        }
    }

    // Asigna a un nombre o a cada parte de una desestructuración (`const { id } = req.params`).
    pub(super) fn bind(&mut self, target: Node<'_>, value: &Value, state: &mut State) {
        match target.kind() {
            IDENTIFIER | SHORTHAND_PATTERN => {
                state.insert(self.text(target).to_owned(), value.clone());
            }
            OBJECT_PATTERN | ARRAY_PATTERN => {
                for part in pattern_parts(target) {
                    self.bind(part, &Value::Data(value.flow()), state);
                }
            }
            ASSIGNMENT_PATTERN => {
                if let Some(left) = target.child_by_field_name(LEFT_FIELD) {
                    self.bind(left, value, state);
                }
            }
            _ => {}
        }
    }

    fn template(&mut self, node: Node<'_>, state: &mut State) -> Value {
        let parts = named_children(node);
        if parts.iter().all(|part| part.kind() != SUBSTITUTION) {
            let written: String = parts
                .iter()
                .filter(|part| part.kind() == STRING_FRAGMENT)
                .map(|part| self.text(*part))
                .collect();
            return Value::Constant(Literal::Text(written));
        }
        Value::Data(
            parts
                .into_iter()
                .filter(|part| part.kind() == SUBSTITUTION)
                .flat_map(named_children)
                .fold(Flow::default(), |flow, inner| {
                    flow.union(&self.expression(inner, state).flow())
                }),
        )
    }

    // `req.query`: lo que trae el pedido; `location.hash`: lo que trae la URL; el resto, lo del objeto.
    fn member(&mut self, node: Node<'_>, state: &mut State) -> Value {
        let (Some(object), Some(property)) = (
            node.child_by_field_name(OBJECT_FIELD),
            node.child_by_field_name(PROPERTY_FIELD),
        ) else {
            return Value::clean();
        };
        if tables::is_browser_source(self.text(node))
            || (tables::is_request_object(self.text(object))
                && tables::is_request_property(self.text(property)))
        {
            return Value::Data(Flow::of(Atom::Source));
        }
        let collection = self.expression(object, state);
        element(
            &collection,
            &Value::Constant(Literal::Text(self.text(property).to_owned())),
        )
    }

    fn assignment(&mut self, node: Node<'_>, state: &mut State) -> Value {
        let (Some(left), Some(right)) = (
            node.child_by_field_name(LEFT_FIELD),
            node.child_by_field_name(RIGHT_FIELD),
        ) else {
            return Value::clean();
        };
        let assigned = self.expression(right, state);
        if left.kind() == MEMBER
            && left
                .child_by_field_name(PROPERTY_FIELD)
                .is_some_and(|property| tables::is_html_property(self.text(property)))
        {
            self.sinks.push(sink_at(
                left,
                self.text(left),
                SinkKind::Html,
                vec![assigned.flow()],
            ));
            return assigned;
        }
        let value = if node.kind() == AUGMENTED {
            let previous = self.expression(left, state);
            Value::Data(previous.flow().union(&assigned.flow()))
        } else {
            assigned.clone()
        };
        if matches!(left.kind(), IDENTIFIER | OBJECT_PATTERN | ARRAY_PATTERN) {
            self.bind(left, &value, state);
        }
        assigned
    }

    fn binary(&mut self, node: Node<'_>, state: &mut State) -> Value {
        let left = node
            .child_by_field_name(LEFT_FIELD)
            .map_or_else(Value::clean, |inner| self.expression(inner, state));
        let right = node
            .child_by_field_name(RIGHT_FIELD)
            .map_or_else(Value::clean, |inner| self.expression(inner, state));
        let operator = node
            .child_by_field_name(OPERATOR_FIELD)
            .map(|written| self.text(written))
            .unwrap_or_default();
        let comparable = operator.trim_end_matches('=');
        if let Some(folded) = left.literal().zip(right.literal()).and_then(|(one, two)| {
            fold_binary(operator, one, two).or_else(|| fold_binary(comparable, one, two))
        }) {
            return Value::Constant(folded);
        }
        if operator == PLUS || KEEPS_OPERANDS.contains(&operator) {
            Value::Data(left.flow().union(&right.flow()))
        } else {
            Value::clean()
        }
    }

    fn object(&mut self, node: Node<'_>, state: &mut State) -> Value {
        let mut entries = Vec::new();
        for part in named_children(node) {
            match part.kind() {
                PAIR => {
                    let key = part
                        .child_by_field_name(KEY_FIELD)
                        .map(|key| self.text(key).trim_matches(['"', '\'']).to_owned())
                        .unwrap_or_default();
                    let value = part
                        .child_by_field_name(VALUE_FIELD)
                        .map_or_else(Value::clean, |value| self.expression(value, state));
                    entries.push((key, value));
                }
                SHORTHAND => {
                    let name = self.text(part).to_owned();
                    let value = state.get(&name).cloned().unwrap_or_else(Value::clean);
                    entries.push((name, value));
                }
                _ => {
                    let spread = self.expression(part, state);
                    entries.push((String::new(), spread));
                }
            }
        }
        Value::Map(entries)
    }

    // `dangerouslySetInnerHTML={{ __html: valor }}`: lo que va en `__html` es HTML crudo.
    fn jsx_attribute(&mut self, node: Node<'_>, state: &mut State) -> Value {
        let parts = named_children(node);
        let (Some(name), Some(value)) = (parts.first(), parts.get(1)) else {
            return Value::clean();
        };
        let rendered = self.expression(*value, state);
        if self.text(*name) == DANGEROUS_HTML {
            let html = element(&rendered, &Value::Constant(Literal::Text(HTML_KEY.to_owned())));
            self.sinks
                .push(sink_at(*name, DANGEROUS_HTML, SinkKind::Html, vec![html.flow()]));
        }
        rendered
    }
}

// Lo que se asigna en `{ a, b: c }` o `[a, b]`: en un par, el lado del valor.
fn pattern_parts(pattern: Node<'_>) -> Vec<Node<'_>> {
    named_children(pattern)
        .into_iter()
        .filter_map(|part| {
            if part.kind() == PAIR_PATTERN {
                part.child_by_field_name(VALUE_FIELD)
            } else {
                Some(part)
            }
        })
        .collect()
}
