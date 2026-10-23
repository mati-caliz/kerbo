use tree_sitter::Node;

use super::model::Model;
use super::qualified::is_builtin_named;
use crate::lint::python::Report;
use crate::lint::python::ast::{
    BLOCK, CALL, CLASS, COMMENT, DECORATED, FUNCTION, FUNCTION_FIELD, NOT_OPERATOR, named, unparenthesized,
};
use crate::rules::PythonRule;

const IF: &str = "if_statement";
const ELIF: &str = "elif_clause";
const ELSE: &str = "else_clause";
const RAISE: &str = "raise_statement";
const BOOLEAN: &str = "boolean_operator";
const CONDITION_FIELD: &str = "condition";
const CONSEQUENCE_FIELD: &str = "consequence";
const BODY_FIELD: &str = "body";
const ALTERNATIVE_FIELD: &str = "alternative";
const CAUSE_FIELD: &str = "cause";
const TYPE_CHECKS: [&str; 3] = ["isinstance", "issubclass", "callable"];
const JUMPS: [&str; 3] = ["return_statement", "break_statement", "continue_statement"];
const OPAQUE: [&str; 3] = [FUNCTION, CLASS, DECORATED];
// `is_builtin_exception` de TRY004: las excepciones genéricas que no dicen que el tipo está mal.
const GENERIC_EXCEPTIONS: [&str; 15] = [
    "ArithmeticError",
    "AssertionError",
    "AttributeError",
    "BufferError",
    "EOFError",
    "Exception",
    "ImportError",
    "LookupError",
    "MemoryError",
    "NameError",
    "ReferenceError",
    "RuntimeError",
    "SyntaxError",
    "SystemError",
    "ValueError",
];

pub struct TypeChecks<'model, 'tree> {
    pub model: &'model Model<'tree>,
    pub source: &'model [u8],
}

impl TypeChecks<'_, '_> {
    // TRY004: un `if isinstance(...)` (o `issubclass`, `callable`) que lanza una excepción genérica en
    // vez de `TypeError`.
    pub fn reports(&self, statement: Node<'_>) -> Vec<Report> {
        let outer_test = statement
            .parent()
            .filter(|block| block.kind() == BLOCK)
            .and_then(|block| block.parent())
            .map(|holder| {
                if matches!(holder.kind(), ELIF | ELSE) {
                    holder.parent().unwrap_or(holder)
                } else {
                    holder
                }
            })
            .filter(|holder| holder.kind() == IF)
            .and_then(|outer| outer.child_by_field_name(CONDITION_FIELD));
        let passes = |test: Option<Node<'_>>| test.is_some_and(|test| self.is_type_check(test));
        if outer_test.is_some_and(|test| !self.is_type_check(test))
            || !passes(statement.child_by_field_name(CONDITION_FIELD))
        {
            return Vec::new();
        }
        let mut reports = self.raises(statement.child_by_field_name(CONSEQUENCE_FIELD));
        let mut cursor = statement.walk();
        let clauses: Vec<Node<'_>> = statement
            .children_by_field_name(ALTERNATIVE_FIELD, &mut cursor)
            .collect();
        for clause in clauses {
            if clause.kind() == ELIF && !passes(clause.child_by_field_name(CONDITION_FIELD)) {
                break;
            }
            let body = clause
                .child_by_field_name(CONSEQUENCE_FIELD)
                .or_else(|| clause.child_by_field_name(BODY_FIELD));
            reports.extend(self.raises(body));
        }
        reports
    }

    fn is_type_check(&self, test: Node<'_>) -> bool {
        let test = unparenthesized(test);
        match test.kind() {
            BOOLEAN | NOT_OPERATOR => named(test).iter().all(|operand| self.is_type_check(*operand)),
            CALL => test.child_by_field_name(FUNCTION_FIELD).is_some_and(|function| {
                TYPE_CHECKS
                    .iter()
                    .any(|check| is_builtin_named(self.model, function, self.source, check))
            }),
            _ => false,
        }
    }

    fn raises(&self, body: Option<Node<'_>>) -> Vec<Report> {
        let mut reports = Vec::new();
        let statements = body.map(named).unwrap_or_default();
        for statement in statements
            .into_iter()
            .filter(|statement| statement.kind() != COMMENT)
        {
            if jumps(statement) {
                break;
            }
            if statement.kind() == RAISE && self.raises_generic(statement) {
                reports.push(Report::at(PythonRule::TRY004, statement, ""));
            }
        }
        reports
    }

    fn raises_generic(&self, statement: Node<'_>) -> bool {
        let cause = statement.child_by_field_name(CAUSE_FIELD);
        let Some(exception) = named(statement)
            .into_iter()
            .find(|child| child.kind() != COMMENT && cause.is_none_or(|cause| cause.id() != child.id()))
        else {
            return false;
        };
        let class = if exception.kind() == CALL {
            exception.child_by_field_name(FUNCTION_FIELD).unwrap_or(exception)
        } else {
            exception
        };
        GENERIC_EXCEPTIONS
            .iter()
            .any(|builtin| is_builtin_named(self.model, class, self.source, builtin))
    }
}

// `has_control_flow`: un `return`, `break` o `continue` en algún lugar de la sentencia, sin entrar a
// funciones ni clases.
fn jumps(statement: Node<'_>) -> bool {
    let mut pending = vec![statement];
    while let Some(node) = pending.pop() {
        if JUMPS.contains(&node.kind()) {
            return true;
        }
        if !OPAQUE.contains(&node.kind()) {
            pending.extend(named(node));
        }
    }
    false
}
