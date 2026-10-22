use tree_sitter::Node;

use super::calls::{call, call_target};
use super::statements::Interpreter;
use super::tables;
use crate::lint::taint::folding::{fold_binary, fold_unary};
use crate::lint::taint::{Atom, Flow, Literal, State, Value, element};
use crate::syntax::named_children;

const IDENTIFIER: &str = "identifier";
const BLANK: &str = "_";
const INTERPRETED: &str = "interpreted_string_literal";
const RAW: &str = "raw_string_literal";
const INTEGER: &str = "int_literal";
const TRUE: &str = "true";
const FALSE: &str = "false";
const SELECTOR: &str = "selector_expression";
const INDEX: &str = "index_expression";
const SLICE: &str = "slice_expression";
const CALL: &str = "call_expression";
const CONVERSION: &str = "type_conversion_expression";
const BINARY: &str = "binary_expression";
const UNARY: &str = "unary_expression";
const COMPOSITE: &str = "composite_literal";
const LITERAL_VALUE: &str = "literal_value";
const KEYED: &str = "keyed_element";
const FUNCTION_LITERAL: &str = "func_literal";
const NIL: &str = "nil";
const WRAPPERS: &[&str] = &[
    "parenthesized_expression",
    "type_assertion_expression",
    "literal_element",
];
const OPERAND_FIELD: &str = "operand";
const FIELD_FIELD: &str = "field";
const INDEX_FIELD: &str = "index";
const LEFT_FIELD: &str = "left";
const RIGHT_FIELD: &str = "right";
const OPERATOR_FIELD: &str = "operator";
const BODY_FIELD: &str = "body";
const KEY_FIELD: &str = "key";
const VALUE_FIELD: &str = "value";
const QUOTES: &[char] = &['"', '`'];
const KEEPS_OPERANDS: &[&str] = &["+"];
const DIGIT_SEPARATOR: char = '_';

// El valor de un literal: un texto (sin interpretar los escapes), un entero o un booleano.
pub fn literal_of(node: Node<'_>, source: &[u8]) -> Option<Literal> {
    let text = node.utf8_text(source).unwrap_or_default();
    match node.kind() {
        INTERPRETED | RAW => Some(Literal::Text(text.trim_matches(QUOTES).to_owned())),
        INTEGER => text
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
            SELECTOR => self.selector(node, state),
            INDEX => {
                let collection =
                    field(OPERAND_FIELD).map_or_else(Value::clean, |inner| self.expression(inner, state));
                let index =
                    field(INDEX_FIELD).map_or_else(Value::clean, |inner| self.expression(inner, state));
                element(&collection, &index)
            }
            SLICE => Value::Data(
                field(OPERAND_FIELD).map_or_else(Flow::default, |inner| self.expression(inner, state).flow()),
            ),
            CALL | CONVERSION => call(self, node, state),
            BINARY => self.binary(node, state),
            UNARY => {
                let operand =
                    field(OPERAND_FIELD).map_or_else(Value::clean, |inner| self.expression(inner, state));
                let operator = field(OPERATOR_FIELD)
                    .map(|written| self.text(written))
                    .unwrap_or_default();
                let folded = operand
                    .literal()
                    .map(|literal| fold_unary(operator, literal).map_or_else(Value::clean, Value::Constant));
                folded.unwrap_or(operand)
            }
            COMPOSITE => field(BODY_FIELD).map_or_else(Value::clean, |body| self.composite(body, state)),
            LITERAL_VALUE => self.composite(node, state),
            kind if WRAPPERS.contains(&kind) => named_children(node)
                .first()
                .map_or_else(Value::clean, |inner| self.expression(*inner, state)),
            FUNCTION_LITERAL => self.closure(node, state),
            NIL => Value::clean(),
            _ => Value::Data(
                named_children(node)
                    .into_iter()
                    .fold(Flow::default(), |flow, child| {
                        flow.union(&self.expression(child, state).flow())
                    }),
            ),
        }
    }

    // `x = valor` sobre una variable o una posición de un mapa o arreglo; los campos no se siguen.
    pub(super) fn store(&mut self, target: Node<'_>, value: Value, state: &mut State) {
        match target.kind() {
            IDENTIFIER if self.text(target) != BLANK => {
                state.insert(self.text(target).to_owned(), value);
            }
            INDEX => {
                let (Some(collection), Some(index)) = (
                    target.child_by_field_name(OPERAND_FIELD),
                    target.child_by_field_name(INDEX_FIELD),
                ) else {
                    return;
                };
                let key = self.expression(index, state);
                let name = self.text(collection).to_owned();
                let updated = match (state.remove(&name), key.literal()) {
                    (Some(Value::Map(mut entries)), Some(Literal::Text(written))) => {
                        entries.retain(|(existing, _)| existing != written);
                        entries.push((written.clone(), value));
                        Value::Map(entries)
                    }
                    (previous, _) => Value::Data(
                        previous
                            .map(|old| old.flow())
                            .unwrap_or_default()
                            .union(&value.flow()),
                    ),
                };
                state.insert(name, updated);
            }
            _ => {}
        }
    }

    // El tipo de una expresión, si se sabe: una variable declarada, una conexión recién abierta o
    // lo que devuelve un método de gorm.
    pub(super) fn type_of(&self, node: Node<'_>) -> Option<String> {
        match node.kind() {
            IDENTIFIER => self.types.get(self.text(node)).map(str::to_owned),
            UNARY => node
                .child_by_field_name(OPERAND_FIELD)
                .and_then(|inner| self.type_of(inner)),
            kind if WRAPPERS.contains(&kind) => named_children(node)
                .first()
                .and_then(|inner| self.type_of(*inner)),
            SELECTOR => {
                let owner = self.type_of(node.child_by_field_name(OPERAND_FIELD)?)?;
                let field = self.text(node.child_by_field_name(FIELD_FIELD)?);
                self.types.field(&owner, field).map(str::to_owned)
            }
            CALL => {
                let target = call_target(self, node)?;
                tables::returned_type(&target.qualified, target.receiver_type.as_deref(), &target.method)
            }
            _ => None,
        }
    }

    // `r.URL`: lo de un pedido; `c.Request` de gin: el pedido; `os.Args`: una variable de paquete.
    fn selector(&mut self, node: Node<'_>, state: &mut State) -> Value {
        let (Some(operand), Some(property)) = (
            node.child_by_field_name(OPERAND_FIELD),
            node.child_by_field_name(FIELD_FIELD),
        ) else {
            return Value::clean();
        };
        let name = self.text(property);
        if self
            .type_of(operand)
            .is_some_and(|written| tables::is_context_type(&written))
            && tables::is_context_request(name)
        {
            return Value::Data(Flow::of(Atom::Source));
        }
        if operand.kind() == IDENTIFIER
            && !state.contains_key(self.text(operand))
            && self.imports.package(self.text(operand)).is_some()
        {
            return Value::clean();
        }
        let object = self.expression(operand, state);
        element(&object, &Value::Constant(Literal::Text(name.to_owned())))
    }

    fn binary(&mut self, node: Node<'_>, state: &mut State) -> Value {
        let side = |interpreter: &mut Self, name: &str, current: &mut State| {
            node.child_by_field_name(name)
                .map_or_else(Value::clean, |inner| interpreter.expression(inner, current))
        };
        let left = side(self, LEFT_FIELD, state);
        let right = side(self, RIGHT_FIELD, state);
        let operator = node
            .child_by_field_name(OPERATOR_FIELD)
            .map(|written| self.text(written))
            .unwrap_or_default();
        if let Some(folded) = left
            .literal()
            .zip(right.literal())
            .and_then(|(one, two)| fold_binary(operator, one, two))
        {
            return Value::Constant(folded);
        }
        if KEEPS_OPERANDS.contains(&operator) {
            Value::Data(left.flow().union(&right.flow()))
        } else {
            Value::clean()
        }
    }

    // `T{Campo: x}` y `map[string]string{"k": v}` son un mapa por clave; `[]T{a, b}`, una lista.
    fn composite(&mut self, body: Node<'_>, state: &mut State) -> Value {
        let elements = named_children(body);
        if elements.iter().all(|element| element.kind() != KEYED) {
            return Value::List(
                elements
                    .into_iter()
                    .map(|item| self.expression(item, state))
                    .collect(),
            );
        }
        let mut entries = Vec::new();
        for keyed in elements.into_iter().filter(|element| element.kind() == KEYED) {
            let key = keyed
                .child_by_field_name(KEY_FIELD)
                .map(|key| self.text(key).trim_matches(QUOTES).to_owned())
                .unwrap_or_default();
            let value = keyed
                .child_by_field_name(VALUE_FIELD)
                .map_or_else(Value::clean, |value| self.expression(value, state));
            entries.push((key, value));
        }
        Value::Map(entries)
    }
}
