use tree_sitter::Node;

use crate::rules::ShellRule;
use crate::shell::commands::Command;
use crate::shell::context::{Context, Findings};
use crate::shell::flow::{Reachability, reachability};
use crate::shell::syntax::{
    BODY_FIELD, CASE_ITEM, COMMAND, COMMAND_SUBSTITUTION, COMMENT, COMPOUND, DO_GROUP, ELIF_CLAUSE,
    ELSE_CLAUSE, EXPANSIONS, FUNCTION, IF, LIST, NAME_FIELD, NEGATED, PROGRAM, REDIRECTED, SUBSHELL, TEST,
    ancestors, children, descendants, field_children, is_backgrounded, named_children, text,
};
use crate::shell::words::{braced_content, literal};

const EXIT: &str = "exit";
const EXIT_CODE: &str = "?";
const TEST_COMMAND: &str = "test";
const ECHO: &str = "echo";
const CONDITION_FIELD: &str = "condition";
const PATH_SEPARATOR: char = '/';
const ESCAPE: char = '\\';

// SC2317, SC2329, SC2218, SC2319 y SC2320: lo que en ShellCheck sale del grafo de control.
pub fn check(context: &Context<'_, '_>, findings: &mut Findings) {
    let reached = reachability(context);
    let source = context.source;
    let nodes = descendants(context.root);
    let functions: Vec<Node<'_>> = nodes
        .iter()
        .copied()
        .filter(|node| node.kind() == FUNCTION)
        .collect();
    for function in &functions {
        check_function(*function, &reached, source, findings);
    }
    for statement in reached.dead_statements() {
        check_dead_statement(*statement, &reached, source, findings);
    }
    if !functions.is_empty() {
        for command in nodes.iter().filter(|node| node.kind() == COMMAND) {
            check_use_before_definition(*command, &functions, source, findings);
        }
    }
    for expansion in nodes.iter().filter(|node| EXPANSIONS.contains(&node.kind())) {
        let is_in_dead_code = ancestors(*expansion).any(|ancestor| reached.is_dead(ancestor));
        if !is_in_dead_code && braced_content(*expansion, source) == EXIT_CODE {
            check_overwritten_exit_code(*expansion, source, findings);
        }
    }
}

fn check_function(function: Node<'_>, reached: &Reachability<'_>, source: &[u8], findings: &mut Findings) {
    let inside_unreachable = ancestors(function)
        .filter(|ancestor| ancestor.kind() == FUNCTION)
        .any(|ancestor| !reached.is_invoked(ancestor));
    if !reached.is_invoked(function) && !inside_unreachable {
        let name = function
            .child_by_field_name(NAME_FIELD)
            .map_or("", |name| text(name, source));
        findings.report(ShellRule::Sc2329, function, name);
    }
}

// ShellCheck sólo marca cada `T_Pipeline`, y no si algo que lo contiene también está muerto: `a && b`, `! a`
// y `a &` son nodos por encima del pipeline, así que muertos no avisan.
fn check_dead_statement(
    statement: Node<'_>,
    reached: &Reachability<'_>,
    source: &[u8],
    findings: &mut Findings,
) {
    if [LIST, NEGATED].contains(&statement.kind()) || is_backgrounded(statement, source) {
        return;
    }
    let inside_unreachable = ancestors(statement).any(|ancestor| {
        reached.is_dead(ancestor) || (ancestor.kind() == FUNCTION && !reached.is_invoked(ancestor))
    });
    if !inside_unreachable {
        findings.report(ShellRule::Sc2317, statement, "");
    }
}

// SC2218: `checkUseBeforeDefinition`. Una definición alcanza a un uso si está en el nivel de arriba del
// script, después del uso y sin un `exit` en el medio.
fn check_use_before_definition(
    command: Node<'_>,
    functions: &[Node<'_>],
    source: &[u8],
    findings: &mut Findings,
) {
    let Some(name) = Command::of(command)
        .and_then(|command| command.first_word())
        .filter(|word| !text(*word, source).contains(ESCAPE))
        .and_then(|word| literal(word, source))
    else {
        return;
    };
    if ancestors(command).any(|ancestor| [FUNCTION, SUBSHELL].contains(&ancestor.kind())) {
        return;
    }
    let definitions: Vec<&Node<'_>> = functions
        .iter()
        .filter(|function| {
            function
                .child_by_field_name(NAME_FIELD)
                .is_some_and(|defined| text(defined, source) == name)
        })
        .collect();
    let defined_later = definitions.iter().any(|definition| {
        is_top_level(**definition)
            && definition.start_byte() > command.end_byte()
            && !exits_between(command, **definition, functions, source)
    });
    let redefines = always_runs(command)
        && definitions
            .iter()
            .any(|definition| definition.end_byte() <= command.start_byte());
    if defined_later && !redefines {
        findings.report(ShellRule::Sc2218, command, &name);
    }
}

fn is_top_level(node: Node<'_>) -> bool {
    node.parent().is_some_and(|parent| parent.kind() == PROGRAM)
}

// Que corre siempre que corre el script: no está en una rama ni del lado derecho de `&&` o `||`.
fn always_runs(command: Node<'_>) -> bool {
    let mut current = command;
    for ancestor in ancestors(command) {
        let is_conditional = match ancestor.kind() {
            LIST => named_children(ancestor).first() != Some(&current),
            IF | ELIF_CLAUSE | ELSE_CLAUSE | CASE_ITEM | DO_GROUP => true,
            _ => false,
        };
        if is_conditional {
            return false;
        }
        current = ancestor;
    }
    true
}

// Un `exit` o una función que lo tiene, entre el uso y la definición: por ese camino la definición no
// llega a correr.
fn exits_between(command: Node<'_>, definition: Node<'_>, functions: &[Node<'_>], source: &[u8]) -> bool {
    let Some(root) = ancestors(command).last() else {
        return false;
    };
    let exiting: Vec<&str> = functions
        .iter()
        .filter(|function| {
            descendants(**function)
                .into_iter()
                .any(|node| is_exit(node, source))
        })
        .filter_map(|function| function.child_by_field_name(NAME_FIELD))
        .map(|name| text(name, source))
        .collect();
    descendants(root).into_iter().any(|node| {
        let calls_exiting = || {
            Command::of(node)
                .and_then(|inner| inner.name(source))
                .is_some_and(|name| exiting.contains(&name.as_str()))
        };
        node.start_byte() >= command.end_byte()
            && node.end_byte() <= definition.start_byte()
            && (is_exit(node, source) || calls_exiting())
    })
}

fn is_exit(node: Node<'_>, source: &[u8]) -> bool {
    Command::of(node).is_some_and(|command| command.is_exactly(EXIT, source))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ExitCodeSource {
    Unconditional,
    Conditional,
}

// SC2319 y SC2320: `checkOverwrittenExitCode`, mirando el comando que corrió justo antes.
fn check_overwritten_exit_code(expansion: Node<'_>, source: &[u8], findings: &mut Findings) {
    let Some((previous, relation)) = previous_command(expansion) else {
        return;
    };
    let finishing = finishing_commands(previous);
    if finishing.iter().all(|command| is_condition(*command, source)) {
        if relation == ExitCodeSource::Conditional {
            findings.report(ShellRule::Sc2319, expansion, EXIT_CODE);
        }
    } else if finishing.iter().all(|command| is_printing(*command, source)) {
        findings.report(ShellRule::Sc2320, expansion, EXIT_CODE);
    }
}

// Sube desde la expansión hasta el primer ancestro que tenga algo corrido antes en su bloque.
fn previous_command(expansion: Node<'_>) -> Option<(Node<'_>, ExitCodeSource)> {
    let mut current = expansion;
    for ancestor in ancestors(expansion) {
        match ancestor.kind() {
            LIST => {
                if let [left, right] = named_children(ancestor)[..]
                    && right == current
                {
                    return Some((left, ExitCodeSource::Conditional));
                }
            }
            IF => {
                let conditions: Vec<Node<'_>> = field_children(ancestor, CONDITION_FIELD)
                    .into_iter()
                    .filter(Node::is_named)
                    .collect();
                if conditions.contains(&current) {
                    return None;
                }
                let first_in_body = children(ancestor)
                    .into_iter()
                    .find(|child| child.is_named() && !conditions.contains(child));
                if first_in_body == Some(current) {
                    return conditions.last().map(|last| (*last, ExitCodeSource::Conditional));
                }
                return sibling_before(current);
            }
            FUNCTION | PROGRAM | COMMAND_SUBSTITUTION | SUBSHELL => return sibling_before(current),
            _ => {
                if let Some(found) = sibling_before(current)
                    && is_statement_list(ancestor)
                {
                    return Some(found);
                }
            }
        }
        current = ancestor;
    }
    None
}

fn is_statement_list(node: Node<'_>) -> bool {
    [COMPOUND, DO_GROUP, ELSE_CLAUSE, ELIF_CLAUSE, CASE_ITEM].contains(&node.kind())
}

fn sibling_before(node: Node<'_>) -> Option<(Node<'_>, ExitCodeSource)> {
    let mut previous = node.prev_named_sibling();
    while let Some(candidate) = previous {
        if candidate.kind() != COMMENT {
            return Some((candidate, ExitCodeSource::Unconditional));
        }
        previous = candidate.prev_named_sibling();
    }
    None
}

// Los comandos que pueden haber dejado el `$?`: en `a && b` son los dos.
fn finishing_commands(statement: Node<'_>) -> Vec<Node<'_>> {
    match statement.kind() {
        REDIRECTED => statement
            .child_by_field_name(BODY_FIELD)
            .map_or_else(|| vec![statement], finishing_commands),
        LIST => named_children(statement)
            .into_iter()
            .flat_map(finishing_commands)
            .collect(),
        _ => vec![statement],
    }
}

fn is_condition(node: Node<'_>, source: &[u8]) -> bool {
    node.kind() == TEST || Command::of(node).is_some_and(|command| command.is_exactly(TEST_COMMAND, source))
}

// ShellCheck le atribuye el código de `printf`, y el de lo que va detrás de `command`, a una palabra y no
// al comando: de los que imprimen, sólo reconoce un `echo` directo.
fn is_printing(node: Node<'_>, source: &[u8]) -> bool {
    Command::of(node)
        .and_then(|command| command.literal_name(source))
        .is_some_and(|name| name.rsplit(PATH_SEPARATOR).next() == Some(ECHO))
}
