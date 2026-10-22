use std::collections::HashMap;

use tree_sitter::Node;

use super::java::{NAME_FIELD, text};
use super::java_chains::{Chain, origin_chain};
use super::java_null_conditions::{asserted_present, branch_states, is_logical, without_cast};
use super::java_receivers::Variables;
use crate::syntax::{named_children, one_based};

pub(super) const IDENTIFIER: &str = "identifier";
pub(super) const INVOCATION: &str = "method_invocation";
const FIELD_ACCESS: &str = "field_access";
const ARRAY_ACCESS: &str = "array_access";
pub(super) const PARENTHESIZED: &str = "parenthesized_expression";
pub(super) const UNARY: &str = "unary_expression";
pub(super) const BINARY: &str = "binary_expression";
const TERNARY: &str = "ternary_expression";
pub(super) const INSTANCEOF: &str = "instanceof_expression";
const ASSIGNMENT: &str = "assignment_expression";
pub(super) const NULL: &str = "null_literal";
const SKIPPED_EXPRESSIONS: &[&str] = &["lambda_expression", "class_body"];
const BLOCK: &str = "block";
const LOCAL: &str = "local_variable_declaration";
const VARIABLE_DECLARATOR: &str = "variable_declarator";
const EXPRESSION_STATEMENT: &str = "expression_statement";
const IF: &str = "if_statement";
const WHILE: &str = "while_statement";
const DO: &str = "do_statement";
const FOR: &str = "for_statement";
const ENHANCED_FOR: &str = "enhanced_for_statement";
const TRY_STATEMENTS: &[&str] = &["try_statement", "try_with_resources_statement"];
const CATCH: &str = "catch_clause";
const FINALLY: &str = "finally_clause";
const SWITCH: &str = "switch_expression";
const SYNCHRONIZED: &str = "synchronized_statement";
const LABELED: &str = "labeled_statement";
const ASSERT: &str = "assert_statement";
const EXITS: &[&str] = &[
    "return_statement",
    "throw_statement",
    "break_statement",
    "continue_statement",
    "yield_statement",
];
pub(super) const OBJECT_FIELD: &str = "object";
const ARRAY_FIELD: &str = "array";
pub(super) const ARGUMENTS_FIELD: &str = "arguments";
const CONDITION_FIELD: &str = "condition";
const CONSEQUENCE_FIELD: &str = "consequence";
const ALTERNATIVE_FIELD: &str = "alternative";
const BODY_FIELD: &str = "body";
const INIT_FIELD: &str = "init";
const UPDATE_FIELD: &str = "update";
const VALUE_FIELD: &str = "value";
pub(super) const LEFT_FIELD: &str = "left";
pub(super) const RIGHT_FIELD: &str = "right";
pub(super) const OPERATOR_FIELD: &str = "operator";
pub(super) const OPERAND_FIELD: &str = "operand";
pub(super) const NOT: &str = "!";
pub(super) const AND: &str = "&&";
pub(super) const OR: &str = "||";
pub(super) const EQUAL: &str = "==";
pub(super) const NOT_EQUAL: &str = "!=";
pub(super) const COMMENTS: &[&str] = &["line_comment", "block_comment"];

// Un uso de una variable que quedaría sin proteger si la llamada que le dio su valor devolviera
// null: el método o campo que se usa y dónde.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dereference {
    pub name: String,
    pub line: usize,
    pub column: usize,
}

// Una variable asignada desde una llamada (`Cache cache = manager.getCache(nombre)`) y los usos
// que no tienen un chequeo de null por delante en ningún camino.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NullFlow {
    pub origin: Chain,
    pub dereferences: Vec<Dereference>,
}

// Las variables que podrían ser null en un punto del método, con la asignación que lo causó.
pub(super) type State = HashMap<String, usize>;

pub(super) struct Analysis<'source, 'variables> {
    source: &'source [u8],
    variables: &'variables Variables,
    flows: Vec<NullFlow>,
}

// Recorre el cuerpo de un método como lo haría IntelliJ, sin compilar: cada variable asignada
// desde una llamada se supone posiblemente null, los chequeos (`x != null`, `x == null` con
// salida, `instanceof`, `requireNonNull`, `assertThat(x).isNotNull()`, `hasText`) la dan por
// presente en el camino que corresponde, y el primer uso sin chequeo en cada camino se anota.
// Las lambdas y las clases anónimas no se recorren.
pub fn null_flows(body: Node<'_>, source: &[u8], variables: &Variables) -> Vec<NullFlow> {
    let mut analysis = Analysis {
        source,
        variables,
        flows: Vec::new(),
    };
    analysis.statement(body, State::new());
    analysis
        .flows
        .into_iter()
        .filter(|flow| !flow.dereferences.is_empty())
        .collect()
}

pub(super) fn join(first: Option<State>, second: Option<State>) -> Option<State> {
    match (first, second) {
        (Some(mut merged), Some(other)) => {
            for (name, origin) in other {
                merged.entry(name).or_insert(origin);
            }
            Some(merged)
        }
        (one, None) | (None, one) => one,
    }
}

impl Analysis<'_, '_> {
    pub(super) fn text(&self, node: Node<'_>) -> &str {
        text(node, self.source)
    }

    fn statement(&mut self, node: Node<'_>, state: State) -> Option<State> {
        let kind = node.kind();
        let field = |name: &str| node.child_by_field_name(name);
        match kind {
            BLOCK => named_children(node)
                .into_iter()
                .filter(|child| !COMMENTS.contains(&child.kind()))
                .try_fold(state, |current, child| self.statement(child, current)),
            LOCAL => Some(self.declaration(node, state)),
            EXPRESSION_STATEMENT => Some(self.expression_statement(node, state)),
            IF => {
                let (when_true, when_false) = branch_states(self, field(CONDITION_FIELD)?, state);
                let taken = match field(CONSEQUENCE_FIELD) {
                    Some(branch) => self.statement(branch, when_true),
                    None => Some(when_true),
                };
                let other = match field(ALTERNATIVE_FIELD) {
                    Some(branch) => self.statement(branch, when_false),
                    None => Some(when_false),
                };
                join(taken, other)
            }
            WHILE | FOR => {
                let mut current = state;
                if let Some(init) = field(INIT_FIELD) {
                    current = self.expression(init, current);
                }
                let (when_true, when_false) = match field(CONDITION_FIELD) {
                    Some(condition) => branch_states(self, condition, current),
                    None => (current.clone(), current),
                };
                let after_body = field(BODY_FIELD).and_then(|body| self.statement(body, when_true));
                if let (Some(update), Some(after)) = (field(UPDATE_FIELD), after_body.clone()) {
                    self.expression(update, after);
                }
                join(Some(when_false), after_body)
            }
            DO => {
                let after_body = field(BODY_FIELD).and_then(|body| self.statement(body, state.clone()));
                let after = join(Some(state), after_body)?;
                field(CONDITION_FIELD).map(|condition| branch_states(self, condition, after.clone()).1)
            }
            ENHANCED_FOR => {
                let current = match field(VALUE_FIELD) {
                    Some(iterable) => self.expression(iterable, state),
                    None => state,
                };
                let after_body = field(BODY_FIELD).and_then(|body| self.statement(body, current.clone()));
                join(Some(current), after_body)
            }
            SYNCHRONIZED | LABELED => named_children(node)
                .into_iter()
                .filter(|child| !COMMENTS.contains(&child.kind()))
                .try_fold(state, |current, child| {
                    if child.kind() == BLOCK || child.kind().ends_with("statement") {
                        self.statement(child, current)
                    } else {
                        Some(self.expression(child, current))
                    }
                }),
            ASSERT => named_children(node)
                .into_iter()
                .find(|child| !COMMENTS.contains(&child.kind()))
                .map(|condition| branch_states(self, condition, state.clone()).0)
                .or(Some(state)),
            SWITCH => Some(self.expression(node, state)),
            kind if TRY_STATEMENTS.contains(&kind) => self.try_statement(node, state),
            kind if EXITS.contains(&kind) => {
                for child in named_children(node) {
                    self.expression(child, state.clone());
                }
                None
            }
            _ => Some(state),
        }
    }

    fn try_statement(&mut self, node: Node<'_>, state: State) -> Option<State> {
        let body = node
            .child_by_field_name(BODY_FIELD)
            .and_then(|body| self.statement(body, state.clone()));
        let mut result = body;
        let mut finally = None;
        for clause in named_children(node) {
            match clause.kind() {
                CATCH => {
                    let handled = clause
                        .child_by_field_name(BODY_FIELD)
                        .and_then(|block| self.statement(block, state.clone()));
                    result = join(result, handled);
                }
                FINALLY => finally = Some(clause),
                _ => {}
            }
        }
        match finally.and_then(|clause| {
            named_children(clause)
                .into_iter()
                .find(|child| child.kind() == BLOCK)
        }) {
            // El finally puede correr después de una excepción en cualquier punto del try: arranca
            // también con lo que había a la entrada.
            Some(block) => {
                let entry = join(Some(state), result.clone()).unwrap_or_default();
                self.statement(block, entry).and(result)
            }
            None => result,
        }
    }

    fn declaration(&mut self, node: Node<'_>, mut state: State) -> State {
        for declarator in named_children(node)
            .into_iter()
            .filter(|child| child.kind() == VARIABLE_DECLARATOR)
        {
            let Some(name) = declarator.child_by_field_name(NAME_FIELD) else {
                continue;
            };
            let variable = self.text(name).to_owned();
            match declarator.child_by_field_name(VALUE_FIELD) {
                Some(value) => {
                    state = self.expression(value, state);
                    state = self.assign(variable, value, state);
                }
                None => {
                    state.remove(&variable);
                }
            }
        }
        state
    }

    fn assign(&mut self, variable: String, written: Node<'_>, mut state: State) -> State {
        state.remove(&variable);
        let value = without_cast(written);
        let origin = (value.kind() == INVOCATION)
            .then(|| origin_chain(value, self.source, self.variables))
            .flatten();
        if let Some(origin) = origin {
            state.insert(variable, self.flows.len());
            self.flows.push(NullFlow {
                origin,
                dereferences: Vec::new(),
            });
        }
        state
    }

    fn expression_statement(&mut self, node: Node<'_>, state: State) -> State {
        let Some(expression) = named_children(node).into_iter().next() else {
            return state;
        };
        let mut after = self.expression(expression, state);
        if let Some(asserted) = asserted_present(expression, self.source) {
            after.remove(self.text(asserted));
        }
        after
    }

    // El estado después de evaluar la expresión, anotando los usos sin chequeo.
    pub(super) fn expression(&mut self, node: Node<'_>, state: State) -> State {
        let kind = node.kind();
        if SKIPPED_EXPRESSIONS.contains(&kind) {
            return state;
        }
        if kind == TERNARY {
            let Some(condition) = node.child_by_field_name(CONDITION_FIELD) else {
                return state;
            };
            let (when_true, when_false) = branch_states(self, condition, state);
            let taken = match node.child_by_field_name(CONSEQUENCE_FIELD) {
                Some(branch) => self.expression(branch, when_true),
                None => when_true,
            };
            let other = match node.child_by_field_name(ALTERNATIVE_FIELD) {
                Some(branch) => self.expression(branch, when_false),
                None => when_false,
            };
            return join(Some(taken), Some(other)).unwrap_or_default();
        }
        if kind == BINARY && is_logical(node) {
            let (when_true, when_false) = branch_states(self, node, state);
            return join(Some(when_true), Some(when_false)).unwrap_or_default();
        }
        if kind == ASSIGNMENT {
            let (Some(left), Some(right)) = (
                node.child_by_field_name(LEFT_FIELD),
                node.child_by_field_name(RIGHT_FIELD),
            ) else {
                return state;
            };
            let after = self.expression(right, state);
            return if left.kind() == IDENTIFIER {
                self.assign(self.text(left).to_owned(), right, after)
            } else {
                self.expression(left, after)
            };
        }
        let mut current = state;
        for child in named_children(node) {
            current = self.expression(child, current);
        }
        self.dereference(node, current)
    }

    fn dereference(&mut self, node: Node<'_>, mut state: State) -> State {
        let (receiver, used) = match node.kind() {
            INVOCATION | FIELD_ACCESS => (
                node.child_by_field_name(OBJECT_FIELD),
                node.child_by_field_name(if node.kind() == INVOCATION {
                    NAME_FIELD
                } else {
                    "field"
                }),
            ),
            ARRAY_ACCESS => (
                node.child_by_field_name(ARRAY_FIELD),
                node.child_by_field_name(ARRAY_FIELD),
            ),
            _ => (None, None),
        };
        let (Some(receiver), Some(used)) = (receiver.filter(|receiver| receiver.kind() == IDENTIFIER), used)
        else {
            return state;
        };
        if let Some(origin) = state.remove(self.text(receiver)) {
            let start = used.start_position();
            let name = self.text(used).to_owned();
            if let Some(flow) = self.flows.get_mut(origin) {
                flow.dereferences.push(Dereference {
                    name,
                    line: one_based(start.row),
                    column: one_based(start.column),
                });
            }
        }
        state
    }
}
