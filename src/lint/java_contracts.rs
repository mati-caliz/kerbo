use tree_sitter::Node;

use super::java::{BODY_FIELD, PARAMETERS_FIELD, text};
use super::java_contract_interpreter::Interpreter;
use super::java_null_inference::{accepts_null, parameter_name, parameter_type_name};
use crate::syntax::named_children;

const RETURN: &str = "return_statement";
const EXPRESSION_STATEMENT: &str = "expression_statement";
const INVOCATION: &str = "method_invocation";
const COMMENTS: &[&str] = &["line_comment", "block_comment"];
const BOOLEAN: &str = "boolean";
// El mismo tope que IntelliJ: los contratos que pasan de ahí se recortan.
const MAX_CONTRACTS: usize = 10;

// Lo que un contrato pide de un argumento o lo que se sabe de él en una llamada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Constraint {
    Null,
    NotNull,
    True,
    False,
}

impl Constraint {
    #[must_use]
    pub const fn negate(self) -> Self {
        match self {
            Self::Null => Self::NotNull,
            Self::NotNull => Self::Null,
            Self::True => Self::False,
            Self::False => Self::True,
        }
    }
}

// Un estado de los parámetros: `None` es que el contrato no pide nada de ese parámetro.
pub type Clause = Vec<Option<Constraint>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Outcome {
    Value(Constraint),
    Fail,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Contract {
    pub(super) state: Clause,
    pub(super) outcome: Outcome,
}

pub(super) struct FormalParameter {
    pub(super) name: String,
    pub(super) primitive: bool,
    pub(super) boolean: bool,
}

// Las cláusulas `... -> fail` que IntelliJ infiere del cuerpo de un método que nadie puede
// redefinir. `not_null` dice qué parámetros ya da por no nulos: un contrato que pide `null` ahí se
// descarta.
pub fn fail_clauses(method: Node<'_>, source: &[u8], not_null: &[bool]) -> Vec<Clause> {
    let Some(body) = method.child_by_field_name(BODY_FIELD) else {
        return Vec::new();
    };
    let statements = statements_of(body);
    if statements.is_empty() || matches!(statements.as_slice(), [only] if delegates(*only)) {
        return Vec::new();
    }
    let interpreter = Interpreter {
        parameters: formal_parameters(method, source),
        source,
    };
    let start = vec![None; interpreter.parameters.len()];
    let fails: Vec<Clause> = interpreter
        .statements(vec![start], &statements)
        .into_iter()
        .filter(|contract| {
            !contract
                .state
                .iter()
                .zip(not_null)
                .any(|(constraint, not_null)| *not_null && *constraint == Some(Constraint::Null))
        })
        .take(MAX_CONTRACTS)
        .filter(|contract| contract.outcome == Outcome::Fail)
        .map(|contract| contract.state)
        .collect();
    if fails.iter().any(|clause| clause.iter().all(Option::is_none)) {
        return Vec::new();
    }
    fails
}

// Un `return` o una llamada sola son una delegación, de la que IntelliJ no saca `fail` propios.
fn delegates(statement: Node<'_>) -> bool {
    statement.kind() == RETURN
        || statement.kind() == EXPRESSION_STATEMENT
            && named_children(statement)
                .first()
                .is_some_and(|expression| expression.kind() == INVOCATION)
}

pub(super) fn statements_of(block: Node<'_>) -> Vec<Node<'_>> {
    named_children(block)
        .into_iter()
        .filter(|statement| !COMMENTS.contains(&statement.kind()))
        .collect()
}

fn formal_parameters(method: Node<'_>, source: &[u8]) -> Vec<FormalParameter> {
    method
        .child_by_field_name(PARAMETERS_FIELD)
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .filter_map(parameter_name)
        .map(|name| {
            let primitive = !accepts_null(name);
            FormalParameter {
                name: text(name, source).to_owned(),
                primitive,
                boolean: primitive && parameter_type_name(name, source) == BOOLEAN,
            }
        })
        .collect()
}

pub(super) fn with_constraint(state: &Clause, index: usize, constraint: Constraint) -> Option<Clause> {
    let current = *state.get(index)?;
    if current == Some(constraint.negate()) {
        return None;
    }
    let mut changed = state.clone();
    *changed.get_mut(index)? = Some(constraint);
    Some(changed)
}

pub(super) fn returning(contracts: &[Contract], value: Constraint) -> Vec<Clause> {
    contracts
        .iter()
        .filter(|contract| contract.outcome == Outcome::Value(value))
        .map(|contract| contract.state.clone())
        .collect()
}

pub(super) fn all_returning(states: Vec<Clause>, outcome: Outcome) -> Vec<Contract> {
    states
        .into_iter()
        .map(|state| Contract { state, outcome })
        .collect()
}

pub(super) fn negated(contract: Contract) -> Option<Contract> {
    match contract.outcome {
        Outcome::Value(value @ (Constraint::True | Constraint::False)) => Some(Contract {
            state: contract.state,
            outcome: Outcome::Value(value.negate()),
        }),
        Outcome::Value(Constraint::Null | Constraint::NotNull) | Outcome::Fail | Outcome::Other => None,
    }
}

pub(super) const fn boolean_outcome(value: bool) -> Outcome {
    Outcome::Value(if value {
        Constraint::True
    } else {
        Constraint::False
    })
}
