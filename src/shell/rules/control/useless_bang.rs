use tree_sitter::Node;

use crate::rules::ShellRule;
use crate::shell::commands::Command;
use crate::shell::context::{Context, Findings};
use crate::shell::quoting::is_condition;
use crate::shell::syntax::{
    ARITHMETIC_FOR, BODY_FIELD, COMPOUND, CONDITION_FIELD, DO_GROUP, ELIF_CLAUSE, ELSE_CLAUSE, FOR, FUNCTION,
    IF, NEGATED, PROGRAM, SUBSHELL, WHILE, field_children, is_arithmetic_command, named_children,
};

const BANG: &str = "!";

// SC2251: `checkUselessBang`, un `!` que con `set -e` no corta el script porque no es el resultado de nada.
pub fn check(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    if !context.has_set_e {
        return;
    }
    for statement in non_returning(node, context.source) {
        if statement.kind() == NEGATED
            && !is_condition(statement, context.source)
            && !negates_misread_group(statement, context.source)
        {
            findings.report(ShellRule::Sc2251, statement, BANG);
        }
    }
}

// `while ! {`: tree-sitter parte el grupo en comandos sueltos y el `!` queda afuera de la condición.
fn negates_misread_group(negated: Node<'_>, source: &[u8]) -> bool {
    named_children(negated)
        .first()
        .and_then(|inner| Command::of(*inner))
        .is_some_and(|command| command.is_misread_group(source))
}

// `getNonReturningCommands`: lo que no es el valor de retorno del bloque que lo contiene.
fn non_returning<'tree>(node: Node<'tree>, source: &[u8]) -> Vec<Node<'tree>> {
    let without_last = |mut statements: Vec<Node<'tree>>| {
        statements.pop();
        statements
    };
    match node.kind() {
        PROGRAM | SUBSHELL => without_last(named_children(node)),
        COMPOUND if !is_arithmetic_command(node, source) => {
            if node.parent().is_some_and(|parent| parent.kind() == FUNCTION) {
                without_last(named_children(node))
            } else {
                named_children(node)
            }
        }
        WHILE => {
            let mut statements = without_last(named_conditions(node));
            statements.extend(loop_body(node));
            statements
        }
        FOR | ARITHMETIC_FOR => loop_body(node),
        IF | ELIF_CLAUSE => {
            let conditions = named_conditions(node);
            let body: Vec<Node<'tree>> = named_children(node)
                .into_iter()
                .filter(|child| {
                    !conditions.contains(child) && ![ELIF_CLAUSE, ELSE_CLAUSE].contains(&child.kind())
                })
                .collect();
            let mut statements = without_last(conditions);
            statements.extend(body);
            statements
        }
        ELSE_CLAUSE => named_children(node),
        _ => Vec::new(),
    }
}

fn named_conditions(node: Node<'_>) -> Vec<Node<'_>> {
    field_children(node, CONDITION_FIELD)
        .into_iter()
        .filter(Node::is_named)
        .collect()
}

fn loop_body(node: Node<'_>) -> Vec<Node<'_>> {
    node.child_by_field_name(BODY_FIELD)
        .filter(|body| body.kind() == DO_GROUP)
        .map_or_else(Vec::new, named_children)
}
