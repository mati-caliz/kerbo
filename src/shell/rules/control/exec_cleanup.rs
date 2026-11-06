use tree_sitter::Node;

use crate::rules::ShellRule;
use crate::shell::commands::Command;
use crate::shell::context::{Context, Findings};
use crate::shell::flow::has_option;
use crate::shell::syntax::{
    ARITHMETIC_FOR, ASSIGNMENT, ASSIGNMENTS, BODY_FIELD, COMPOUND, CONDITION_FIELD, DO_GROUP, ELIF_CLAUSE,
    ELSE_CLAUSE, FOR, IF, PROGRAM, REDIRECTED, WHILE, field_children, is_arithmetic_command, is_backgrounded,
    named_children, text,
};
use crate::shell::word_pieces::{literal_string, pieces};

const EXECFAIL: &str = "execfail";
const EXEC: &str = "exec";
const NAMED_DESCRIPTOR_OPEN: char = '{';
const CLEANUP_COMMANDS: &[&str] = &[":", "echo", "exit", "printf", "return"];

// SC2093: `checkSpuriousExec`.
pub fn check(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let Some((statements, in_loop)) = statement_list(node, context.source) else {
        return;
    };
    let source = context.source;
    let mut kept = statements;
    while kept.last().is_some_and(|last| is_cleanup(*last, source)) {
        kept.pop();
    }
    let checked = if in_loop {
        kept.len()
    } else {
        kept.len().saturating_sub(1)
    };
    let execs: Vec<Node<'_>> = kept
        .into_iter()
        .take(checked)
        .filter_map(|statement| exec_with_arguments(statement, source))
        .collect();
    if !execs.is_empty() && !has_option(context, EXECFAIL) {
        for exec in execs {
            findings.report(ShellRule::Sc2093, exec, text(exec, source));
        }
    }
}

fn statement_list<'tree>(node: Node<'tree>, source: &[u8]) -> Option<(Vec<Node<'tree>>, bool)> {
    match node.kind() {
        PROGRAM | ELSE_CLAUSE => Some((named_children(node), false)),
        COMPOUND if !is_arithmetic_command(node, source) => Some((named_children(node), false)),
        DO_GROUP => {
            let in_loop = node
                .parent()
                .is_some_and(|parent| [WHILE, FOR, ARITHMETIC_FOR].contains(&parent.kind()));
            Some((named_children(node), in_loop))
        }
        IF | ELIF_CLAUSE => {
            let condition = field_children(node, CONDITION_FIELD);
            let body = named_children(node)
                .into_iter()
                .filter(|child| {
                    !condition.contains(child) && ![ELIF_CLAUSE, ELSE_CLAUSE].contains(&child.kind())
                })
                .collect();
            Some((body, false))
        }
        _ => None,
    }
}

fn is_cleanup(statement: Node<'_>, source: &[u8]) -> bool {
    if is_backgrounded(statement, source) {
        return false;
    }
    let inner = unredirected(statement);
    if [ASSIGNMENT, ASSIGNMENTS].contains(&inner.kind()) {
        return true;
    }
    Command::of(inner).is_some_and(|command| {
        (command.words.is_empty() && !command.assignments.is_empty())
            || command
                .name(source)
                .is_some_and(|name| CLEANUP_COMMANDS.contains(&name.as_str()))
    })
}

fn unredirected(statement: Node<'_>) -> Node<'_> {
    if statement.kind() == REDIRECTED {
        statement.child_by_field_name(BODY_FIELD).unwrap_or(statement)
    } else {
        statement
    }
}

fn exec_with_arguments<'tree>(statement: Node<'tree>, source: &[u8]) -> Option<Node<'tree>> {
    if is_backgrounded(statement, source) {
        return None;
    }
    let command = Command::of(unredirected(statement))?;
    let [name, argument, ..] = command.words[..] else {
        return None;
    };
    let names_descriptor = text(argument, source).starts_with(NAMED_DESCRIPTOR_OPEN);
    (!names_descriptor && literal_string(&pieces(name, source)).as_deref() == Some(EXEC))
        .then_some(command.node)
}
