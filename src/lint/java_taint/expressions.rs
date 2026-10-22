use tree_sitter::Node;

use super::closures::{Callback, anonymous_class, argument, lambda};
use super::collections::{collection_call, empty_collection};
use super::constants::literal_of;
use super::receivers::{chain_root, declared_type, sink_at, sink_receiver};
use super::statements::{BODY_FIELD, Interpreter, SWITCH, SWITCH_LABEL};
use super::tables;
use crate::lint::java::NAME_FIELD;
use crate::lint::java_receivers::{Receiver, simple_type_name};
use crate::lint::taint::folding::{fold_binary, fold_text_method, fold_unary, truth};
use crate::lint::taint::{Atom, Flow, Literal, SinkReceiver, State, TaintCall, Value};
use crate::syntax::named_children;

const IDENTIFIER: &str = "identifier";
const PARENTHESIZED: &str = "parenthesized_expression";
const CAST: &str = "cast_expression";
const BINARY: &str = "binary_expression";
const UNARY: &str = "unary_expression";
const TERNARY: &str = "ternary_expression";
const ASSIGNMENT: &str = "assignment_expression";
const INVOCATION: &str = "method_invocation";
const CREATION: &str = "object_creation_expression";
const ARRAY_ACCESS: &str = "array_access";
const SWITCH_RULE: &str = "switch_rule";
const LAMBDA: &str = "lambda_expression";
const CLEAN_EXPRESSIONS: &[&str] = &[
    "method_reference",
    "class_literal",
    "this",
    "instanceof_expression",
    "update_expression",
    "null_literal",
    "field_access",
];
const COMMENTS: &[&str] = &["line_comment", "block_comment"];
const OBJECT_FIELD: &str = "object";
const ARGUMENTS_FIELD: &str = "arguments";
const TYPE_FIELD: &str = "type";
const VALUE_FIELD: &str = "value";
const ARRAY_FIELD: &str = "array";
const LEFT_FIELD: &str = "left";
const RIGHT_FIELD: &str = "right";
const OPERATOR_FIELD: &str = "operator";
const OPERAND_FIELD: &str = "operand";
const CONDITION_FIELD: &str = "condition";
const CONSEQUENCE_FIELD: &str = "consequence";
const ALTERNATIVE_FIELD: &str = "alternative";
const CLASS_BODY: &str = "class_body";
const PLUS: &str = "+";
const ASSIGN: &str = "=";

impl Interpreter<'_, '_> {
    pub(super) fn expression(&mut self, node: Node<'_>, state: &mut State) -> Value {
        let field = |name: &str| node.child_by_field_name(name);
        if let Some(literal) = literal_of(node, self.source) {
            return Value::Constant(literal);
        }
        match node.kind() {
            IDENTIFIER => state.get(self.text(node)).cloned().unwrap_or_else(Value::clean),
            PARENTHESIZED => named_children(node)
                .into_iter()
                .find(|child| !COMMENTS.contains(&child.kind()))
                .map_or_else(Value::clean, |inner| self.expression(inner, state)),
            CAST => field(VALUE_FIELD).map_or_else(Value::clean, |inner| self.expression(inner, state)),
            BINARY => self.binary(node, state),
            UNARY => {
                let operand =
                    field(OPERAND_FIELD).map_or_else(Value::clean, |inner| self.expression(inner, state));
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
                let decided = truth(condition.literal());
                let mut branch = |name: &str, current: &mut State| {
                    field(name).map_or_else(Value::clean, |inner| self.expression(inner, current))
                };
                match decided {
                    Some(true) => branch(CONSEQUENCE_FIELD, state),
                    Some(false) => branch(ALTERNATIVE_FIELD, state),
                    None => {
                        let taken = branch(CONSEQUENCE_FIELD, state);
                        taken.join(branch(ALTERNATIVE_FIELD, state))
                    }
                }
            }
            ASSIGNMENT => self.assignment(node, state),
            INVOCATION => self.invocation(node, state),
            CREATION => self.creation(node, state),
            ARRAY_ACCESS => {
                let array =
                    field(ARRAY_FIELD).map_or_else(Value::clean, |inner| self.expression(inner, state));
                Value::Data(array.flow())
            }
            SWITCH => self.switch_value(node, state),
            LAMBDA => lambda(self, node, state, &Value::clean()),
            kind if CLEAN_EXPRESSIONS.contains(&kind) => Value::clean(),
            _ => Value::Data(self.all_children(node, state)),
        }
    }

    fn all_children(&mut self, node: Node<'_>, state: &mut State) -> Flow {
        named_children(node)
            .into_iter()
            .filter(|child| !COMMENTS.contains(&child.kind()))
            .fold(Flow::default(), |flow, child| {
                flow.union(&self.expression(child, state).flow())
            })
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
        if let Some(folded) = left
            .literal()
            .zip(right.literal())
            .and_then(|(one, two)| fold_binary(operator, one, two))
        {
            return Value::Constant(folded);
        }
        if operator == PLUS {
            Value::Data(left.flow().union(&right.flow()))
        } else {
            Value::clean()
        }
    }

    fn assignment(&mut self, node: Node<'_>, state: &mut State) -> Value {
        let (Some(left), Some(right)) = (
            node.child_by_field_name(LEFT_FIELD),
            node.child_by_field_name(RIGHT_FIELD),
        ) else {
            return Value::clean();
        };
        let assigned = self.expression(right, state);
        let operator = node
            .child_by_field_name(OPERATOR_FIELD)
            .map_or(ASSIGN, |written| self.text(written));
        let target = match left.kind() {
            IDENTIFIER => Some(left),
            ARRAY_ACCESS => left
                .child_by_field_name(ARRAY_FIELD)
                .filter(|array| array.kind() == IDENTIFIER),
            _ => None,
        };
        if let Some(target) = target {
            let name = self.text(target).to_owned();
            let replaces = operator == ASSIGN && left.kind() == IDENTIFIER;
            let value = match state.remove(&name) {
                Some(previous) if !replaces => Value::Data(previous.flow().union(&assigned.flow())),
                _ => assigned.clone(),
            };
            state.insert(name, value);
        }
        assigned
    }

    fn arguments(&mut self, node: Node<'_>, state: &mut State, callback: &Callback) -> Vec<Value> {
        node.child_by_field_name(ARGUMENTS_FIELD)
            .map(named_children)
            .unwrap_or_default()
            .into_iter()
            .filter(|argument| !COMMENTS.contains(&argument.kind()))
            .map(|written| argument(self, written, state, callback))
            .collect()
    }

    fn invocation(&mut self, node: Node<'_>, state: &mut State) -> Value {
        let name = node
            .child_by_field_name(NAME_FIELD)
            .map(|written| self.text(written))
            .unwrap_or_default();
        let object = node.child_by_field_name(OBJECT_FIELD);
        let receiver = object.map(|inner| self.expression(inner, state));
        let callback = Callback {
            input: receiver
                .as_ref()
                .filter(|_| tables::passes_receiver_to_callback(name))
                .map_or_else(Value::clean, |value| Value::Data(value.flow())),
            keeps_result: tables::keeps_callback_result(name),
        };
        let arguments = self.arguments(node, state, &callback);
        let followed = object
            .filter(|inner| inner.kind() == IDENTIFIER)
            .map(|inner| self.text(inner))
            .filter(|variable| matches!(state.get(*variable), Some(Value::List(_) | Value::Map(_))));
        if let Some(variable) = followed
            && let Some(collection) = state.get_mut(variable)
        {
            return collection_call(collection, name, &arguments);
        }
        if let Some(Literal::Text(written)) = receiver.as_ref().and_then(Value::literal) {
            let constants: Option<Vec<Literal>> = arguments
                .iter()
                .map(|argument| argument.literal().cloned())
                .collect();
            if let Some(folded) = constants.and_then(|constants| fold_text_method(written, name, &constants))
            {
                return Value::Constant(folded);
            }
        }
        if object
            .and_then(|inner| declared_type(self, inner))
            .is_some_and(|declared| tables::is_source(&declared, name))
        {
            return Value::Data(Flow::of(Atom::Source));
        }
        let receiver_flow = receiver.as_ref().map(Value::flow).unwrap_or_default();
        let argument_flows: Vec<Flow> = arguments.iter().map(Value::flow).collect();
        if tables::might_be_method_sink(name)
            && let Some(sink_receiver) = object.and_then(|inner| sink_receiver(self, inner))
        {
            self.sinks
                .push(sink_at(node, name, sink_receiver, argument_flows.clone()));
        }
        if tables::absorbs_arguments(name)
            && let Some(root) = object.and_then(|inner| chain_root(self, inner))
        {
            let absorbed = argument_flows.iter().fold(Flow::default(), Flow::union);
            let previous = state.remove(&root).map(|value| value.flow()).unwrap_or_default();
            state.insert(root, Value::Data(previous.union(&absorbed)));
        }
        let written_owner = object
            .filter(|inner| inner.kind() == IDENTIFIER)
            .map(|inner| self.text(inner));
        if tables::gives_clean_result(written_owner, name) {
            return Value::clean();
        }
        if tables::escapes_html(name) {
            let carried = argument_flows.iter().fold(Flow::default(), Flow::union);
            return Value::Data(carried.escaped_for_html());
        }
        let call_receiver = match object {
            Some(inner) if inner.kind() == CREATION => inner
                .child_by_field_name(TYPE_FIELD)
                .map_or(Receiver::Unknown, |written| {
                    Receiver::Type(simple_type_name(self.text(written)))
                }),
            _ => self.imported.receiver(node, self.source, self.variables),
        };
        self.calls.push(TaintCall {
            name: name.to_owned(),
            receiver: call_receiver,
            receiver_flow,
            arguments: argument_flows,
        });
        Value::Data(Flow::of(Atom::Call(self.calls.len().saturating_sub(1))))
    }

    fn creation(&mut self, node: Node<'_>, state: &mut State) -> Value {
        let created = node
            .child_by_field_name(TYPE_FIELD)
            .map(|written| simple_type_name(self.text(written)))
            .unwrap_or_default();
        let arguments = self.arguments(node, state, &Callback::opaque());
        if let Some(body) = named_children(node)
            .into_iter()
            .find(|child| child.kind() == CLASS_BODY)
        {
            anonymous_class(self, body, state);
            return Value::clean();
        }
        if tables::constructor_sink(&created).is_some() {
            let flows = arguments.iter().map(Value::flow).collect();
            self.sinks.push(sink_at(
                node,
                &created,
                SinkReceiver::Constructor(created.clone()),
                flows,
            ));
            return Value::clean();
        }
        if arguments.is_empty()
            && let Some(collection) = empty_collection(&created)
        {
            return collection;
        }
        Value::Data(
            arguments
                .iter()
                .fold(Flow::default(), |flow, argument| flow.union(&argument.flow())),
        )
    }

    fn switch_value(&mut self, node: Node<'_>, state: &mut State) -> Value {
        let body = node.child_by_field_name(BODY_FIELD);
        let rules: Vec<Node<'_>> = body
            .map(named_children)
            .unwrap_or_default()
            .into_iter()
            .filter(|rule| rule.kind() == SWITCH_RULE)
            .collect();
        if rules.is_empty() {
            if let Some(after) = self.switch_statement(node, state.clone()) {
                *state = after;
            }
            return Value::clean();
        }
        Value::Data(
            rules
                .into_iter()
                .flat_map(named_children)
                .filter(|part| part.kind() != SWITCH_LABEL)
                .fold(Flow::default(), |flow, part| {
                    flow.union(&self.expression(part, state).flow())
                }),
        )
    }
}
