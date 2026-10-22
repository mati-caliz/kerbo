use tree_sitter::Node;

use super::calls::{attribute, call, qualified};
use super::collections::{assign_subscript, subscript};
use super::operators::{binary, boolean, comparison, conditional};
use super::statements::Interpreter;
use crate::lint::taint::folding::{fold_binary, fold_unary};
use crate::lint::taint::{Flow, Literal, SinkKind, SinkReceiver, State, TaintSink, Value};
use crate::syntax::{named_children, one_based};

const IDENTIFIER: &str = "identifier";
const ATTRIBUTE: &str = "attribute";
const CALL: &str = "call";
const SUBSCRIPT: &str = "subscript";
const STRING: &str = "string";
const STRING_CONTENT: &str = "string_content";
const INTERPOLATION: &str = "interpolation";
const INTEGER: &str = "integer";
const TRUE: &str = "true";
const FALSE: &str = "false";
const BINARY: &str = "binary_operator";
const UNARY: &str = "unary_operator";
const BOOLEAN: &str = "boolean_operator";
const NOT: &str = "not_operator";
const COMPARISON: &str = "comparison_operator";
const CONDITIONAL: &str = "conditional_expression";
const PARENTHESIZED: &str = "parenthesized_expression";
const LIST: &str = "list";
const TUPLE: &str = "tuple";
const DICTIONARY: &str = "dictionary";
const PAIR: &str = "pair";
const ASSIGNMENT: &str = "assignment";
const AUGMENTED: &str = "augmented_assignment";
const NAMED: &str = "named_expression";
const PATTERN_LISTS: &[&str] = &["pattern_list", "tuple_pattern", "list_pattern", TUPLE];
const CLEAN_EXPRESSIONS: &[&str] = &["lambda", "none", "float", "ellipsis"];
const FUNCTION_FIELD: &str = "function";
const VALUE_FIELD: &str = "value";
const SUBSCRIPT_FIELD: &str = "subscript";
const LEFT_FIELD: &str = "left";
const RIGHT_FIELD: &str = "right";
const OPERATOR_FIELD: &str = "operator";
const ARGUMENT_FIELD: &str = "argument";
const KEY_FIELD: &str = "key";
const NAME_FIELD: &str = "name";
const QUOTES: &[char] = &['\'', '"'];
const PLUS: &str = "+";
const SLASH: &str = "/";
const METHOD_NAME_FIELD: &str = "attribute";
const PATH_OBJECT: &str = "pathlib.Path";
const NEGATION: &str = "!";
const DIGIT_SEPARATOR: char = '_';
const PYTHON_TRUE: &str = "True";
const PYTHON_FALSE: &str = "False";

// Un literal escrito en un patrón de `case`: un texto entre comillas, un entero o un booleano.
pub fn literal_of_text(written: &str) -> Option<Literal> {
    if let Some(inner) = written
        .strip_prefix(QUOTES)
        .and_then(|rest| rest.strip_suffix(QUOTES))
    {
        return Some(Literal::Text(inner.to_owned()));
    }
    match written {
        PYTHON_TRUE => Some(Literal::Boolean(true)),
        PYTHON_FALSE => Some(Literal::Boolean(false)),
        _ => written.parse().ok().map(Literal::Integer),
    }
}

// Lo que va en el cuerpo de una respuesta de Flask: de una tupla `(cuerpo, estado, headers)`, el
// primer elemento; los headers no son HTML.
pub fn response_body(written: Node<'_>, value: &Value) -> Flow {
    match value {
        Value::List(items) if written.kind() == TUPLE => items.first().map(Value::flow).unwrap_or_default(),
        Value::Constant(_) | Value::Data(_) | Value::List(_) | Value::Map(_) => value.flow(),
    }
}

pub fn sink_at(node: Node<'_>, callee: &str, kind: SinkKind, arguments: Vec<Flow>) -> TaintSink {
    let start = node.start_position();
    TaintSink {
        callee: callee.to_owned(),
        receiver: SinkReceiver::Known(kind),
        arguments,
        line: one_based(start.row),
        column: one_based(start.column),
    }
}

impl Interpreter<'_, '_> {
    pub(super) fn expression(&mut self, node: Node<'_>, state: &mut State) -> Value {
        let field = |name: &str| node.child_by_field_name(name);
        match node.kind() {
            IDENTIFIER => state.get(self.text(node)).cloned().unwrap_or_else(Value::clean),
            STRING => self.string(node, state),
            INTEGER => self.text(node).replace(DIGIT_SEPARATOR, "").parse().map_or_else(
                |_| Value::clean(),
                |number| Value::Constant(Literal::Integer(number)),
            ),
            TRUE => Value::Constant(Literal::Boolean(true)),
            FALSE => Value::Constant(Literal::Boolean(false)),
            PARENTHESIZED => named_children(node)
                .first()
                .map_or_else(Value::clean, |inner| self.expression(*inner, state)),
            BINARY => binary(self, node, state),
            UNARY | NOT => {
                let operand =
                    field(ARGUMENT_FIELD).map_or_else(Value::clean, |inner| self.expression(inner, state));
                let operator = if node.kind() == NOT {
                    NEGATION
                } else {
                    field(OPERATOR_FIELD)
                        .map(|written| self.text(written))
                        .unwrap_or_default()
                };
                operand
                    .literal()
                    .and_then(|literal| fold_unary(operator, literal))
                    .map_or_else(Value::clean, Value::Constant)
            }
            BOOLEAN => boolean(self, node, state),
            COMPARISON => comparison(self, node, state),
            CONDITIONAL => conditional(self, node, state),
            CALL => call(self, node, state),
            ATTRIBUTE => attribute(self, node, state),
            SUBSCRIPT => {
                let collection =
                    field(VALUE_FIELD).map_or_else(Value::clean, |inner| self.expression(inner, state));
                let index =
                    field(SUBSCRIPT_FIELD).map_or_else(Value::clean, |inner| self.expression(inner, state));
                subscript(&collection, &index)
            }
            LIST | TUPLE => Value::List(
                named_children(node)
                    .into_iter()
                    .map(|item| self.expression(item, state))
                    .collect(),
            ),
            DICTIONARY => self.dictionary(node, state),
            ASSIGNMENT => self.assignment(node, state),
            AUGMENTED => self.augmented(node, state),
            NAMED => self.named(node, state),
            kind if CLEAN_EXPRESSIONS.contains(&kind) => Value::clean(),
            _ => Value::Data(
                named_children(node)
                    .into_iter()
                    .fold(Flow::default(), |flow, child| {
                        flow.union(&self.expression(child, state).flow())
                    }),
            ),
        }
    }

    // `x = valor`, también a una tupla o a una posición.
    fn assignment(&mut self, node: Node<'_>, state: &mut State) -> Value {
        let field = |name: &str| node.child_by_field_name(name);
        let assigned = field(RIGHT_FIELD).map_or_else(Value::clean, |right| self.expression(right, state));
        if let Some(target) = field(LEFT_FIELD) {
            self.remember_instance(target, field(RIGHT_FIELD));
            self.bind(target, &assigned, state);
        }
        assigned
    }

    // `x += valor`: lo que tenía más lo nuevo.
    fn augmented(&mut self, node: Node<'_>, state: &mut State) -> Value {
        let field = |name: &str| node.child_by_field_name(name);
        let added = field(RIGHT_FIELD).map_or_else(Value::clean, |right| self.expression(right, state));
        if let Some(target) = field(LEFT_FIELD) {
            let previous = self.expression(target, state);
            let combined = match (previous.literal(), added.literal()) {
                (Some(one), Some(two)) => fold_binary(PLUS, one, two).map_or_else(
                    || Value::Data(previous.flow().union(&added.flow())),
                    Value::Constant,
                ),
                _ => Value::Data(previous.flow().union(&added.flow())),
            };
            self.bind(target, &combined, state);
        }
        Value::clean()
    }

    // `(x := valor)`.
    fn named(&mut self, node: Node<'_>, state: &mut State) -> Value {
        let field = |name: &str| node.child_by_field_name(name);
        let assigned = field(VALUE_FIELD).map_or_else(Value::clean, |value| self.expression(value, state));
        if let Some(target) = field(NAME_FIELD) {
            self.bind(target, &assigned, state);
        }
        assigned
    }

    // Asigna a un nombre, a cada nombre de una tupla, o a una posición de una lista o un diccionario
    // que se siguen.
    pub(super) fn bind(&mut self, target: Node<'_>, value: &Value, state: &mut State) {
        match target.kind() {
            IDENTIFIER => {
                state.insert(self.text(target).to_owned(), value.clone());
            }
            kind if PATTERN_LISTS.contains(&kind) => {
                for part in named_children(target) {
                    self.bind(part, &Value::Data(value.flow()), state);
                }
            }
            SUBSCRIPT => {
                let (Some(collection), Some(index)) = (
                    target.child_by_field_name(VALUE_FIELD),
                    target.child_by_field_name(SUBSCRIPT_FIELD),
                ) else {
                    return;
                };
                let index = self.expression(index, state);
                if collection.kind() == IDENTIFIER
                    && let Some(followed) = state.get_mut(self.text(collection))
                {
                    assign_subscript(followed, &index, value.clone());
                }
            }
            _ => {}
        }
    }

    fn remember_instance(&mut self, target: Node<'_>, value: Option<Node<'_>>) {
        if target.kind() != IDENTIFIER {
            return;
        }
        if let Some(joined) = value.filter(|value| value.kind() == BINARY)
            && joined
                .child_by_field_name(OPERATOR_FIELD)
                .is_some_and(|operator| self.text(operator) == SLASH)
            && joined
                .child_by_field_name(LEFT_FIELD)
                .filter(|left| left.kind() == IDENTIFIER)
                .and_then(|left| self.instances.get(self.text(left)))
                .is_some_and(|created| super::tables::is_path_object(created))
        {
            self.instances
                .insert(self.text(target).to_owned(), PATH_OBJECT.to_owned());
            return;
        }
        let Some(call) = value.filter(|value| value.kind() == CALL) else {
            return;
        };
        let builds_query = call
            .child_by_field_name(FUNCTION_FIELD)
            .filter(|function| function.kind() == ATTRIBUTE)
            .and_then(|function| function.child_by_field_name(METHOD_NAME_FIELD))
            .is_some_and(|name| super::tables::creates_query(self.text(name)));
        if builds_query {
            self.instances.insert(
                self.text(target).to_owned(),
                super::tables::QUERY_OBJECT.to_owned(),
            );
            return;
        }
        let name = self.text(target).to_owned();
        match call
            .child_by_field_name(FUNCTION_FIELD)
            .and_then(|function| qualified(self, function))
        {
            Some(created) => {
                self.instances.insert(name, created);
            }
            None => {
                self.instances.remove(&name);
            }
        }
    }

    fn string(&mut self, node: Node<'_>, state: &mut State) -> Value {
        let parts = named_children(node);
        if parts.iter().all(|part| part.kind() != INTERPOLATION) {
            let written: String = parts
                .iter()
                .filter(|part| part.kind() == STRING_CONTENT)
                .map(|part| self.text(*part))
                .collect();
            return Value::Constant(Literal::Text(written));
        }
        Value::Data(
            parts
                .into_iter()
                .filter(|part| part.kind() == INTERPOLATION)
                .flat_map(named_children)
                .fold(Flow::default(), |flow, inner| {
                    flow.union(&self.expression(inner, state).flow())
                }),
        )
    }

    fn dictionary(&mut self, node: Node<'_>, state: &mut State) -> Value {
        let mut entries = Vec::new();
        let mut followed = true;
        let mut flow = Flow::default();
        for pair in named_children(node) {
            if pair.kind() != PAIR {
                followed = false;
                continue;
            }
            let key = pair
                .child_by_field_name(KEY_FIELD)
                .map_or_else(Value::clean, |key| self.expression(key, state));
            let value = pair
                .child_by_field_name(VALUE_FIELD)
                .map_or_else(Value::clean, |value| self.expression(value, state));
            flow.absorb(&value.flow());
            match key.literal() {
                Some(Literal::Text(written)) => entries.push((written.clone(), value)),
                _ => followed = false,
            }
        }
        if followed {
            Value::Map(entries)
        } else {
            Value::Data(flow)
        }
    }
}
