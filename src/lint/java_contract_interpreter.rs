use tree_sitter::Node;

use super::java::{ARRAY_CREATION, BODY_FIELD, CLASS, IDENTIFIER, OBJECT_CREATION, TYPE_FIELD, text};
use super::java_contracts::{
    Clause, Constraint, Contract, FormalParameter, Outcome, all_returning, boolean_outcome, negated,
    returning, statements_of, with_constraint,
};
use crate::syntax::named_children;

const BLOCK: &str = "block";
const IF: &str = "if_statement";
const WHILE: &str = "while_statement";
const DO: &str = "do_statement";
const THROW: &str = "throw_statement";
const RETURN: &str = "return_statement";
const ASSERT: &str = "assert_statement";
const LOCAL_VARIABLE: &str = "local_variable_declaration";
const BINARY: &str = "binary_expression";
const TERNARY: &str = "ternary_expression";
const PARENTHESIZED: &str = "parenthesized_expression";
const CAST: &str = "cast_expression";
const UNARY: &str = "unary_expression";
const INSTANCEOF: &str = "instanceof_expression";
const THIS: &str = "this";
const NULL: &str = "null_literal";
const TRUE: &str = "true";
const FALSE: &str = "false";
const VARIABLE_DECLARATOR: &str = "variable_declarator";
const PRIMITIVE_TYPES: &[&str] = &["integral_type", "floating_point_type", "boolean_type"];
const NOT_NULL_LITERALS: &[&str] = &[
    "decimal_integer_literal",
    "hex_integer_literal",
    "octal_integer_literal",
    "binary_integer_literal",
    "decimal_floating_point_literal",
    "hex_floating_point_literal",
    "character_literal",
    "string_literal",
    "text_block",
];
const CONDITION_FIELD: &str = "condition";
const CONSEQUENCE_FIELD: &str = "consequence";
const ALTERNATIVE_FIELD: &str = "alternative";
const OPERATOR_FIELD: &str = "operator";
const OPERAND_FIELD: &str = "operand";
const LEFT_FIELD: &str = "left";
const RIGHT_FIELD: &str = "right";
const VALUE_FIELD: &str = "value";
const CONCATENATION: &str = "+";
const EQUALS: &str = "==";
const NOT_EQUALS: &str = "!=";
const AND: &str = "&&";
const OR: &str = "||";
const NOT: &str = "!";
// El mismo tope que IntelliJ: con más estados el método es "demasiado complejo".
const MAX_STATES: usize = 300;

fn literal_constraint(node: Node<'_>) -> Option<Constraint> {
    match node.kind() {
        NULL => Some(Constraint::Null),
        TRUE => Some(Constraint::True),
        FALSE => Some(Constraint::False),
        kind if NOT_NULL_LITERALS.contains(&kind) => Some(Constraint::NotNull),
        _ => None,
    }
}

fn initializes(declaration: Node<'_>) -> bool {
    named_children(declaration)
        .into_iter()
        .filter(|child| child.kind() == VARIABLE_DECLARATOR)
        .any(|declarator| declarator.child_by_field_name(VALUE_FIELD).is_some())
}

// Lo que deja una sentencia: sus contratos y, si la ejecución puede seguir de largo, los estados
// con los que llega a la siguiente.
struct Step {
    contracts: Vec<Contract>,
    next: Option<Vec<Clause>>,
}

impl Step {
    const fn last(contracts: Vec<Contract>) -> Self {
        Self {
            contracts,
            next: None,
        }
    }

    const fn passing(states: Vec<Clause>) -> Self {
        Self {
            contracts: Vec::new(),
            next: Some(states),
        }
    }
}

// Calca el ContractInferenceInterpreter de IntelliJ sobre las sentencias de un método.
pub(super) struct Interpreter<'source> {
    pub(super) parameters: Vec<FormalParameter>,
    pub(super) source: &'source [u8],
}

impl Interpreter<'_> {
    // Una variable inicializada descarta los contratos de lo que sigue.
    pub(super) fn statements(&self, mut states: Vec<Clause>, statements: &[Node<'_>]) -> Vec<Contract> {
        let mut accumulated = Vec::new();
        let mut filtered = false;
        for statement in statements {
            if statement.kind() == LOCAL_VARIABLE {
                filtered |= initializes(*statement);
            }
            let step = self.statement(states, *statement);
            if !filtered {
                accumulated.extend(step.contracts);
            }
            let Some(next) = step.next else {
                break;
            };
            states = next;
        }
        accumulated
    }

    fn statement(&self, states: Vec<Clause>, statement: Node<'_>) -> Step {
        let first_child = named_children(statement).first().copied();
        match statement.kind() {
            BLOCK => Step::last(self.statements(states, &statements_of(statement))),
            IF => self.if_statement(&states, statement),
            WHILE => {
                let condition = self.expression(&states, statement.child_by_field_name(CONDITION_FIELD));
                Step::passing(returning(&condition, Constraint::False))
            }
            THROW => Step::last(all_returning(states, Outcome::Fail)),
            RETURN => Step::last(self.expression(&states, first_child)),
            ASSERT => {
                let condition = self.expression(&states, first_child);
                Step::last(all_returning(
                    returning(&condition, Constraint::False),
                    Outcome::Fail,
                ))
            }
            LOCAL_VARIABLE | CLASS => Step::passing(states),
            DO => Step::last(
                statement
                    .child_by_field_name(BODY_FIELD)
                    .map(|body| self.statements(states, &[body]))
                    .unwrap_or_default(),
            ),
            _ => Step::last(Vec::new()),
        }
    }

    fn if_statement(&self, states: &[Clause], statement: Node<'_>) -> Step {
        let condition = self.expression(states, statement.child_by_field_name(CONDITION_FIELD));
        let mut contracts = statement
            .child_by_field_name(CONSEQUENCE_FIELD)
            .map(|then| self.statements(returning(&condition, Constraint::True), &[then]))
            .unwrap_or_default();
        let false_states = returning(&condition, Constraint::False);
        match statement.child_by_field_name(ALTERNATIVE_FIELD) {
            Some(otherwise) => {
                contracts.extend(self.statements(false_states, &[otherwise]));
                Step::last(contracts)
            }
            None => Step {
                contracts,
                next: Some(false_states),
            },
        }
    }

    fn expression(&self, states: &[Clause], expression: Option<Node<'_>>) -> Vec<Contract> {
        let Some(expression) = expression else {
            return Vec::new();
        };
        if states.is_empty() || states.len() > MAX_STATES {
            return Vec::new();
        }
        match expression.kind() {
            BINARY => self.binary(states, expression),
            TERNARY => self.ternary(states, expression),
            PARENTHESIZED => self.expression(states, named_children(expression).first().copied()),
            CAST if expression
                .child_by_field_name(TYPE_FIELD)
                .is_some_and(|written| !PRIMITIVE_TYPES.contains(&written.kind())) =>
            {
                self.expression(states, expression.child_by_field_name(VALUE_FIELD))
            }
            UNARY if self.operator(expression) == NOT => self
                .expression(states, expression.child_by_field_name(OPERAND_FIELD))
                .into_iter()
                .filter_map(negated)
                .collect(),
            INSTANCEOF => self.instance_check(states, expression),
            OBJECT_CREATION | ARRAY_CREATION | THIS => all_returning(states.to_vec(), Outcome::Other),
            _ => self.value(states, expression),
        }
    }

    fn value(&self, states: &[Clause], expression: Node<'_>) -> Vec<Contract> {
        if let Some(constraint) = literal_constraint(expression) {
            return all_returning(states.to_vec(), Outcome::Value(constraint));
        }
        let Some(index) = self.parameter_index(expression) else {
            return Vec::new();
        };
        let boolean = self
            .parameters
            .get(index)
            .is_some_and(|parameter| parameter.boolean);
        states
            .iter()
            .flat_map(|state| match state.get(index).copied().flatten() {
                Some(known @ (Constraint::True | Constraint::False | Constraint::Null)) => vec![Contract {
                    state: state.clone(),
                    outcome: Outcome::Value(known),
                }],
                _ if boolean => [Constraint::True, Constraint::False]
                    .into_iter()
                    .filter_map(|value| {
                        with_constraint(state, index, value).map(|state| Contract {
                            state,
                            outcome: Outcome::Value(value),
                        })
                    })
                    .collect(),
                _ => vec![Contract {
                    state: state.clone(),
                    outcome: Outcome::Other,
                }],
            })
            .collect()
    }

    fn instance_check(&self, states: &[Clause], expression: Node<'_>) -> Vec<Contract> {
        let Some(index) = expression
            .child_by_field_name(LEFT_FIELD)
            .and_then(|left| self.parameter_index(left))
        else {
            return Vec::new();
        };
        states
            .iter()
            .filter_map(|state| with_constraint(state, index, Constraint::Null))
            .map(|state| Contract {
                state,
                outcome: Outcome::Value(Constraint::False),
            })
            .collect()
    }

    fn ternary(&self, states: &[Clause], expression: Node<'_>) -> Vec<Contract> {
        let condition = self.expression(states, expression.child_by_field_name(CONDITION_FIELD));
        let mut contracts = self.expression(
            &returning(&condition, Constraint::True),
            expression.child_by_field_name(CONSEQUENCE_FIELD),
        );
        contracts.extend(self.expression(
            &returning(&condition, Constraint::False),
            expression.child_by_field_name(ALTERNATIVE_FIELD),
        ));
        contracts
    }

    fn binary(&self, states: &[Clause], expression: Node<'_>) -> Vec<Contract> {
        let left = expression.child_by_field_name(LEFT_FIELD);
        let right = expression.child_by_field_name(RIGHT_FIELD);
        match self.operator(expression) {
            CONCATENATION => all_returning(states.to_vec(), Outcome::Value(Constraint::NotNull)),
            EQUALS => self.equality(states, expression, true),
            NOT_EQUALS => self.equality(states, expression, false),
            AND => self.logical(states, [left, right], true),
            OR => self.logical(states, [left, right], false),
            _ => Vec::new(),
        }
    }

    fn equality(&self, states: &[Clause], expression: Node<'_>, equal: bool) -> Vec<Contract> {
        let (Some(left), Some(right)) = (
            expression.child_by_field_name(LEFT_FIELD),
            expression.child_by_field_name(RIGHT_FIELD),
        ) else {
            return Vec::new();
        };
        let compared = self
            .parameter_index(left)
            .zip(literal_constraint(right))
            .or_else(|| self.parameter_index(right).zip(literal_constraint(left)));
        let Some((index, constraint)) = compared else {
            return Vec::new();
        };
        let pairs = if constraint == Constraint::NotNull {
            if self
                .parameters
                .get(index)
                .is_some_and(|parameter| parameter.primitive)
            {
                Vec::new()
            } else {
                vec![(Constraint::Null, boolean_outcome(!equal))]
            }
        } else {
            vec![
                (constraint, boolean_outcome(equal)),
                (constraint.negate(), boolean_outcome(!equal)),
            ]
        };
        states
            .iter()
            .flat_map(|state| {
                pairs.iter().filter_map(|(required, result)| {
                    with_constraint(state, index, *required).map(|narrowed| Contract {
                        state: narrowed,
                        outcome: *result,
                    })
                })
            })
            .collect()
    }

    // `&&` corta en el primer falso y `||` en el primer verdadero; lo que pasa todos los operandos
    // devuelve lo contrario.
    fn logical(
        &self,
        states: &[Clause],
        operands: [Option<Node<'_>>; 2],
        conjunction: bool,
    ) -> Vec<Contract> {
        let breaking = if conjunction {
            Constraint::False
        } else {
            Constraint::True
        };
        let mut remaining = states.to_vec();
        let mut contracts = Vec::new();
        for operand in operands {
            let results = self.expression(&remaining, operand);
            contracts.extend(
                results
                    .iter()
                    .filter(|contract| contract.outcome == Outcome::Value(breaking))
                    .cloned(),
            );
            remaining = returning(&results, breaking.negate());
        }
        contracts.extend(all_returning(remaining, Outcome::Value(breaking.negate())));
        contracts
    }

    fn operator(&self, expression: Node<'_>) -> &str {
        expression
            .child_by_field_name(OPERATOR_FIELD)
            .map_or("", |operator| text(operator, self.source))
    }

    fn parameter_index(&self, expression: Node<'_>) -> Option<usize> {
        if expression.kind() != IDENTIFIER {
            return None;
        }
        let name = text(expression, self.source);
        self.parameters
            .iter()
            .position(|parameter| parameter.name == name)
    }
}
