use tree_sitter::Node;

use crate::rules::ShellRule;
use crate::shell::commands::Command;
use crate::shell::context::{Context, Findings};
use crate::shell::flow::has_lastpipe;
use crate::shell::syntax::{
    ARITHMETIC_FOR, COMMAND, COMMAND_SUBSTITUTION, FOR, FUNCTION, PIPELINE, REDIRECTED, SUBSHELL, WHILE,
    is_backgrounded, named_children, text,
};

const LOOP_KEYWORDS: &[&str] = &["continue", "break"];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Scope {
    Loop,
    Function,
    Subshell,
}

// SC2104, SC2105 y SC2106: `checkLoopKeywordScope`.
pub fn check(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    if node.kind() != COMMAND {
        return;
    }
    let source = context.source;
    let Some(name) = Command::of(node).and_then(|command| command.name(source)) else {
        return;
    };
    if !LOOP_KEYWORDS.contains(&name.as_str()) {
        return;
    }
    let scopes = enclosing_scopes(node, context);
    let subject = text(node, source);
    if scopes.contains(&Scope::Loop) {
        let closest = scopes.iter().find(|scope| **scope != Scope::Function);
        if closest == Some(&Scope::Subshell) {
            findings.report(ShellRule::Sc2106, node, subject);
        }
    } else if scopes.first() == Some(&Scope::Function) {
        findings.report(ShellRule::Sc2104, node, subject);
    } else {
        findings.report(ShellRule::Sc2105, node, subject);
    }
}

fn enclosing_scopes(command: Node<'_>, context: &Context<'_, '_>) -> Vec<Scope> {
    let lastpipe = has_lastpipe(context);
    let mut scopes = Vec::new();
    let mut current = Some(command);
    while let Some(node) = current {
        if node != command || node.parent().is_some_and(|parent| parent.kind() == PIPELINE) {
            match node.kind() {
                WHILE | FOR | ARITHMETIC_FOR => scopes.push(Scope::Loop),
                FUNCTION => scopes.push(Scope::Function),
                SUBSHELL | COMMAND_SUBSTITUTION => scopes.push(Scope::Subshell),
                _ => {}
            }
        }
        if causes_subshell(node, lastpipe, context.source) {
            scopes.push(Scope::Subshell);
        }
        current = node.parent();
    }
    scopes
}

fn causes_subshell(node: Node<'_>, lastpipe: bool, source: &[u8]) -> bool {
    if is_backgrounded(node, source) {
        return true;
    }
    if node.kind() == REDIRECTED && node.parent().is_some_and(|parent| parent.kind() == PIPELINE) {
        return is_subshell_stage(node, lastpipe);
    }
    if node.parent().is_some_and(|parent| parent.kind() == PIPELINE) {
        return is_subshell_stage(node, lastpipe);
    }
    false
}

fn is_subshell_stage(stage: Node<'_>, lastpipe: bool) -> bool {
    let Some(pipeline) = stage.parent() else {
        return false;
    };
    let stages = named_children(pipeline);
    stages.len() > 1 && (!lastpipe || stages.last() != Some(&stage))
}
