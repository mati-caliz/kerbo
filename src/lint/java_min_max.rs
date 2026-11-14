use tree_sitter::{Node, Point};

use super::java::{IDENTIFIER, NAME_FIELD, VARIABLE_DECLARATOR, text, unparenthesized};
use super::java_receivers::declared_type;
use super::{LintFinding, LintKind, finding_at};
use crate::fix::Fix;
use crate::rules::JavaRule;
use crate::syntax::{descendants, named_children};

const TERNARY: &str = "ternary_expression";
const IF: &str = "if_statement";
const BINARY: &str = "binary_expression";
const UNARY: &str = "unary_expression";
const BLOCK: &str = "block";
const EXPRESSION_STATEMENT: &str = "expression_statement";
const ASSIGNMENT: &str = "assignment_expression";
const RETURN: &str = "return_statement";
const LOCAL_VARIABLE: &str = "local_variable_declaration";
const DECIMAL_INTEGER: &str = "decimal_integer_literal";
const DECIMAL_FLOAT: &str = "decimal_floating_point_literal";
const CONDITION_FIELD: &str = "condition";
const CONSEQUENCE_FIELD: &str = "consequence";
const ALTERNATIVE_FIELD: &str = "alternative";
const LEFT_FIELD: &str = "left";
const RIGHT_FIELD: &str = "right";
const OPERATOR_FIELD: &str = "operator";
const TYPE_FIELD: &str = "type";
const OPERAND_FIELD: &str = "operand";
const PACKAGE: &str = "package_declaration";
const VALUE_FIELD: &str = "value";
const NAME_FIELD_OF_CLASS: &str = "name";
const ASSIGN: &str = "=";
const MINUS: &str = "-";
const LESS_OPERATORS: [&str; 2] = ["<", "<="];
const GREATER_OPERATORS: [&str; 2] = [">", ">="];
const NUMERIC_TYPES: [&str; 4] = ["int", "long", "float", "double"];
const INT: &str = "int";
const LONG: &str = "long";
const FLOAT: &str = "float";
const DOUBLE: &str = "double";
const LONG_SUFFIXES: [char; 2] = ['L', 'l'];
const FLOAT_SUFFIXES: [char; 2] = ['F', 'f'];
const MATH_CLASS: &str = "Math";
const MATH_PACKAGE_DECLARATION: &str = "java.lang";
const MIN: &str = "min";
const MAX: &str = "max";

struct Conditional<'tree> {
    condition: Node<'tree>,
    then_value: Node<'tree>,
    else_value: Node<'tree>,
}

struct Replaced<'tree> {
    reported: Node<'tree>,
    start: Node<'tree>,
    end: Node<'tree>,
    template: Template,
}

enum Template {
    Expression,
    Return,
    Assign(String),
    Declare(String, String),
}

pub fn reported(root: Node<'_>, source: &[u8]) -> Vec<LintFinding> {
    if declares_java_lang_math(root, source) {
        return Vec::new();
    }
    descendants(root)
        .into_iter()
        .filter_map(|node| match node.kind() {
            TERNARY => ternary(node).map(|conditional| {
                let replaced = Replaced {
                    reported: node,
                    start: node,
                    end: node,
                    template: Template::Expression,
                };
                (conditional, replaced)
            }),
            IF => if_model(node, source),
            _ => None,
        })
        .filter_map(|(conditional, replaced)| report(&conditional, &replaced, source))
        .collect()
}

fn declares_java_lang_math(root: Node<'_>, source: &[u8]) -> bool {
    let in_java_lang = named_children(root).into_iter().any(|child| {
        child.kind() == PACKAGE && super::java::qualified_name(child, source) == MATH_PACKAGE_DECLARATION
    });
    in_java_lang
        && descendants(root).into_iter().any(|node| {
            super::java::is_type_declaration(node)
                && node
                    .child_by_field_name(NAME_FIELD_OF_CLASS)
                    .is_some_and(|name| text(name, source) == MATH_CLASS)
        })
}

fn ternary(node: Node<'_>) -> Option<Conditional<'_>> {
    Some(Conditional {
        condition: node.child_by_field_name(CONDITION_FIELD)?,
        then_value: node.child_by_field_name(CONSEQUENCE_FIELD)?,
        else_value: node.child_by_field_name(ALTERNATIVE_FIELD)?,
    })
}

fn if_model<'tree>(statement: Node<'tree>, source: &[u8]) -> Option<(Conditional<'tree>, Replaced<'tree>)> {
    let condition = statement.child_by_field_name(CONDITION_FIELD)?;
    let then_branch = single_statement(statement.child_by_field_name(CONSEQUENCE_FIELD)?)?;
    let else_branch = statement
        .child_by_field_name(ALTERNATIVE_FIELD)
        .and_then(single_statement);
    let model = else_branch.map_or_else(
        || if_alone_model(condition, then_branch, statement, source),
        |else_branch| if_else_model(then_branch, else_branch, statement, source),
    )?;
    Some((
        Conditional {
            condition,
            then_value: model.then_value,
            else_value: model.else_value,
        },
        Replaced {
            reported: statement,
            start: model.start,
            end: model.end,
            template: model.template,
        },
    ))
}

struct BranchModel<'tree> {
    then_value: Node<'tree>,
    else_value: Node<'tree>,
    start: Node<'tree>,
    end: Node<'tree>,
    template: Template,
}

fn if_else_model<'tree>(
    then_branch: Node<'tree>,
    else_branch: Node<'tree>,
    statement: Node<'tree>,
    source: &[u8],
) -> Option<BranchModel<'tree>> {
    if let (Some((then_target, then_value)), Some((else_target, else_value))) = (
        expression_assignment(then_branch),
        expression_assignment(else_branch),
    ) {
        return equivalent(then_target, else_target, source).then(|| BranchModel {
            then_value,
            else_value,
            start: statement,
            end: statement,
            template: Template::Assign(text(then_target, source).to_owned()),
        });
    }
    Some(BranchModel {
        then_value: returned(then_branch)?,
        else_value: returned(else_branch)?,
        start: statement,
        end: statement,
        template: Template::Return,
    })
}

fn if_alone_model<'tree>(
    condition: Node<'tree>,
    then_branch: Node<'tree>,
    statement: Node<'tree>,
    source: &[u8],
) -> Option<BranchModel<'tree>> {
    let assigned_after_previous = expression_assignment(then_branch)
        .zip(statement.prev_named_sibling())
        .and_then(|((target, value), previous)| {
            let assignment = GuardedAssignment {
                condition,
                statement,
                target,
                value,
                previous,
            };
            previously_assigned(&assignment, source).or_else(|| previously_declared(&assignment, source))
        });
    if assigned_after_previous.is_some() {
        return assigned_after_previous;
    }
    let next = statement.next_named_sibling()?;
    Some(BranchModel {
        then_value: returned(then_branch)?,
        else_value: returned(next)?,
        start: statement,
        end: next,
        template: Template::Return,
    })
}

struct GuardedAssignment<'tree> {
    condition: Node<'tree>,
    statement: Node<'tree>,
    target: Node<'tree>,
    value: Node<'tree>,
    previous: Node<'tree>,
}

fn previously_assigned<'tree>(
    assignment: &GuardedAssignment<'tree>,
    source: &[u8],
) -> Option<BranchModel<'tree>> {
    let (else_target, else_value) = expression_assignment(assignment.previous)?;
    let matches = equivalent(assignment.target, else_target, source) && is_recomputable(else_value);
    matches.then(|| BranchModel {
        then_value: assignment.value,
        else_value,
        start: assignment.previous,
        end: assignment.statement,
        template: Template::Assign(text(assignment.target, source).to_owned()),
    })
}

fn previously_declared<'tree>(
    assignment: &GuardedAssignment<'tree>,
    source: &[u8],
) -> Option<BranchModel<'tree>> {
    let (declared, written_type, initializer) = single_local(assignment.previous, source)?;
    let reads_local = [assignment.condition, assignment.value]
        .into_iter()
        .any(|part| mentions(part, &declared, source));
    let matches = text(assignment.target, source) == declared && is_recomputable(initializer) && !reads_local;
    matches.then_some(BranchModel {
        then_value: assignment.value,
        else_value: initializer,
        start: assignment.previous,
        end: assignment.statement,
        template: Template::Declare(written_type, declared),
    })
}

fn single_statement(branch: Node<'_>) -> Option<Node<'_>> {
    if branch.kind() != BLOCK {
        return Some(branch);
    }
    match named_children(branch).as_slice() {
        [only] => Some(*only),
        _ => None,
    }
}

fn expression_assignment(statement: Node<'_>) -> Option<(Node<'_>, Node<'_>)> {
    if statement.kind() != EXPRESSION_STATEMENT {
        return None;
    }
    let expression = named_children(statement).into_iter().next()?;
    let is_plain = expression.kind() == ASSIGNMENT
        && expression
            .child_by_field_name(OPERATOR_FIELD)
            .is_some_and(|operator| operator.kind() == ASSIGN);
    is_plain
        .then(|| {
            Some((
                expression.child_by_field_name(LEFT_FIELD)?,
                expression.child_by_field_name(RIGHT_FIELD)?,
            ))
        })
        .flatten()
}

fn single_local<'tree>(statement: Node<'tree>, source: &[u8]) -> Option<(String, String, Node<'tree>)> {
    if statement.kind() != LOCAL_VARIABLE {
        return None;
    }
    let declarators: Vec<Node<'_>> = named_children(statement)
        .into_iter()
        .filter(|child| child.kind() == VARIABLE_DECLARATOR)
        .collect();
    let [declarator] = declarators.as_slice() else {
        return None;
    };
    Some((
        text(declarator.child_by_field_name(NAME_FIELD)?, source).to_owned(),
        text(statement.child_by_field_name(TYPE_FIELD)?, source).to_owned(),
        declarator.child_by_field_name(VALUE_FIELD)?,
    ))
}

fn returned(statement: Node<'_>) -> Option<Node<'_>> {
    (statement.kind() == RETURN)
        .then(|| named_children(statement).into_iter().next())
        .flatten()
}

fn report(conditional: &Conditional<'_>, replaced: &Replaced<'_>, source: &[u8]) -> Option<LintFinding> {
    let condition = unparenthesized(conditional.condition);
    if condition.kind() != BINARY {
        return None;
    }
    let operator = text(condition.child_by_field_name(OPERATOR_FIELD)?, source);
    let is_less = LESS_OPERATORS.contains(&operator);
    if !is_less && !GREATER_OPERATORS.contains(&operator) {
        return None;
    }
    let left = condition.child_by_field_name(LEFT_FIELD)?;
    let right = condition.child_by_field_name(RIGHT_FIELD)?;
    let left_type = numeric_type(left, source)?;
    if numeric_type(right, source)? != left_type {
        return None;
    }
    let left_is_else = equivalent(left, conditional.else_value, source);
    if !left_is_else && !equivalent(left, conditional.then_value, source) {
        return None;
    }
    let right_value = if left_is_else {
        conditional.then_value
    } else {
        conditional.else_value
    };
    if !equivalent(right, right_value, source) {
        return None;
    }
    let function = if left_is_else ^ is_less { MIN } else { MAX };
    let call = format!(
        "{MATH_CLASS}.{function}({}, {})",
        text(left, source),
        text(right, source)
    );
    let replacement = match &replaced.template {
        Template::Expression => call,
        Template::Return => format!("return {call};"),
        Template::Assign(target) => format!("{target} = {call};"),
        Template::Declare(written_type, name) => format!("{written_type} {name} = {call};"),
    };
    let start: Point = replaced.start.start_position();
    let end: Point = replaced.end.end_position();
    Some(finding_at(
        LintKind::Java(JavaRule::ManualMinMaxCalculation),
        replaced.reported.start_position(),
        function,
        Some(Fix::replace_range(start, end, &replacement, source)),
    ))
}

fn numeric_type<'source>(expression: Node<'_>, source: &'source [u8]) -> Option<&'source str> {
    let expression = unparenthesized(expression);
    match expression.kind() {
        IDENTIFIER => {
            let written = declared_type(expression, text(expression, source), source)?;
            let written = text(written, source);
            NUMERIC_TYPES.iter().find(|numeric| **numeric == written).copied()
        }
        DECIMAL_INTEGER => Some(if text(expression, source).ends_with(LONG_SUFFIXES) {
            LONG
        } else {
            INT
        }),
        DECIMAL_FLOAT => Some(if text(expression, source).ends_with(FLOAT_SUFFIXES) {
            FLOAT
        } else {
            DOUBLE
        }),
        UNARY => {
            let negated = expression
                .child_by_field_name(OPERATOR_FIELD)
                .is_some_and(|operator| text(operator, source) == MINUS);
            negated
                .then(|| numeric_type(expression.child_by_field_name(OPERAND_FIELD)?, source))
                .flatten()
        }
        _ => None,
    }
}

fn equivalent(left: Node<'_>, right: Node<'_>, source: &[u8]) -> bool {
    let canonical = |node: Node<'_>| -> String {
        text(unparenthesized(node), source)
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect()
    };
    canonical(left) == canonical(right)
}

fn is_recomputable(expression: Node<'_>) -> bool {
    let expression = unparenthesized(expression);
    [IDENTIFIER, DECIMAL_INTEGER, DECIMAL_FLOAT].contains(&expression.kind())
        || (expression.kind() == UNARY
            && named_children(expression)
                .iter()
                .all(|operand| is_recomputable(*operand)))
}

fn mentions(node: Node<'_>, name: &str, source: &[u8]) -> bool {
    descendants(node)
        .into_iter()
        .any(|part| part.kind() == IDENTIFIER && text(part, source) == name)
}
