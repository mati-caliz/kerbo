use std::collections::HashMap;

use tree_sitter::Node;

use super::java::{NAME_FIELD, text};
use super::java_chains::origin_chain;
use super::java_null_conditions::{
    asserted_condition, asserted_present, branch_states, condition_asserted, is_logical, without_cast,
};
use super::java_null_keys::{forget, forget_fields, join, seed_nullable_fields, spellings, tracked_key};
use super::java_null_loops::runs_at_least_once;
use super::java_null_uses::{dereference, passed};
use super::java_receivers::{StaticImports, Variables};
use crate::syntax::named_children;

pub use super::java_null_origin::{Dereference, NullFlow, NullOrigin};

pub(super) const IDENTIFIER: &str = "identifier";
pub(super) const INVOCATION: &str = "method_invocation";
const CREATION: &str = "object_creation_expression";
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
const RESOURCES: &str = "resource_specification";
const RESOURCE: &str = "resource";
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

// Las variables que podrían ser null en un punto del método, con la asignación que lo causó.
pub(super) type State = HashMap<String, usize>;

pub(super) struct Analysis<'source, 'context> {
    pub(super) source: &'source [u8],
    pub(super) variables: &'context Variables,
    pub(super) imported: &'context StaticImports,
    pub(super) flows: Vec<NullFlow>,
    pub(super) partners: HashMap<String, String>,
    pub(super) forgettable: Vec<(Vec<String>, NullOrigin)>,
}

// Recorre el cuerpo de un método como lo haría IntelliJ, sin compilar: cada variable asignada
// desde una llamada y cada campo `@Nullable` de las clases que lo encierran se suponen
// posiblemente null, los chequeos (`x != null`, `x == null` con
// salida, `instanceof`, `requireNonNull`, `assertThat(x).isNotNull()`, `hasText`) la dan por
// presente en el camino que corresponde, y el primer uso sin chequeo en cada camino se anota.
// Las lambdas y las clases anónimas no se recorren.
pub fn null_flows(
    method: Node<'_>,
    body: Node<'_>,
    source: &[u8],
    context: (&Variables, &StaticImports),
) -> Vec<NullFlow> {
    let (variables, imported) = context;
    let mut analysis = Analysis {
        source,
        variables,
        imported,
        flows: Vec::new(),
        partners: HashMap::new(),
        forgettable: Vec::new(),
    };
    let entry = seed_nullable_fields(&mut analysis, method);
    analysis.statement(body, entry);
    analysis
        .flows
        .into_iter()
        .filter(|flow| !flow.dereferences.is_empty() || !flow.arguments.is_empty())
        .collect()
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
                if kind == FOR && after_body.is_some() && runs_at_least_once(node, self.source) {
                    return after_body;
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

    fn try_statement(&mut self, node: Node<'_>, mut state: State) -> Option<State> {
        for resource in named_children(node)
            .into_iter()
            .filter(|child| child.kind() == RESOURCES)
            .flat_map(named_children)
            .filter(|child| child.kind() == RESOURCE)
        {
            if let (Some(name), Some(value)) = (
                resource.child_by_field_name(NAME_FIELD),
                resource.child_by_field_name(VALUE_FIELD),
            ) {
                state = self.expression(value, state);
                state = self.assign(self.text(name).to_owned(), value, state);
            }
        }
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
        let keys = spellings(self, variable);
        let value = without_cast(written);
        let copied = tracked_key(self, value).and_then(|source_key| state.get(&source_key).copied());
        for key in &keys {
            state.remove(key);
        }
        let origin = copied.or_else(|| {
            let chain = (value.kind() == INVOCATION)
                .then(|| origin_chain(value, self.source, self.variables))
                .flatten()?;
            self.flows.push(NullFlow {
                origin: NullOrigin::Call(chain),
                dereferences: Vec::new(),
                arguments: Vec::new(),
                passed_before: Vec::new(),
            });
            Some(self.flows.len() - 1)
        });
        if let Some(origin) = origin {
            for key in keys {
                state.insert(key, origin);
            }
        }
        state
    }

    fn expression_statement(&mut self, node: Node<'_>, state: State) -> State {
        let Some(expression) = named_children(node).into_iter().next() else {
            return state;
        };
        match condition_asserted(expression, self.source) {
            Some(condition) => asserted_condition(self, expression, condition, state),
            None => self.expression(expression, state),
        }
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
            return match tracked_key(self, left) {
                Some(variable) => self.assign(variable, right, after),
                None => self.expression(left, after),
            };
        }
        // El receptor se usa antes que los argumentos: en `x.put(k, x.get(k))` el primer uso es `put`.
        let receiver = node
            .child_by_field_name(OBJECT_FIELD)
            .filter(|_| kind == INVOCATION);
        let mut current = state;
        if let Some(object) = receiver {
            current = self.expression(object, current);
            current = dereference(self, node, current);
        }
        for child in named_children(node)
            .into_iter()
            .filter(|child| Some(child.id()) != receiver.map(|object| object.id()))
        {
            current = self.expression(child, current);
        }
        if kind == INVOCATION || kind == CREATION {
            current = passed(self, node, current);
        }
        let asserted = asserted_present(node, self.source);
        if kind == INVOCATION && asserted.is_none() {
            current = forget_fields(self, node, current);
        }
        if let Some(asserted) = asserted.and_then(|argument| tracked_key(self, argument)) {
            forget(&mut current, &asserted);
        }
        if receiver.is_some() {
            current
        } else {
            dereference(self, node, current)
        }
    }
}
