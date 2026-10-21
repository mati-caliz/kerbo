use tree_sitter::Node;

use crate::rules::ShellRule;
use crate::shell::context::{Context, Findings};
use crate::shell::syntax::{
    BODY_FIELD, COMMAND, COMMAND_SUBSTITUTION, DESCRIPTOR_FIELD, DESTINATION_FIELD, FILE_REDIRECT,
    HEREDOC_REDIRECT, HERESTRING, PIPELINE, PROCESS_SUBSTITUTION, REDIRECTED, children, text,
};

const STDERR: &str = "2";
const STDOUT: &str = "1";
const DUPLICATE_OUTPUT: &str = ">&";
const FILE_OUTPUTS: &[&str] = &[">", ">>"];
const REDIRECT_KINDS: &[&str] = &[FILE_REDIRECT, HERESTRING, HEREDOC_REDIRECT];

pub fn check(node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
    let is_outermost_command = node.kind() == COMMAND
        && node.parent().is_none_or(|parent| {
            parent.kind() != REDIRECTED || parent.child_by_field_name(BODY_FIELD) != Some(node)
        });
    if node.kind() != REDIRECTED && !is_outermost_command {
        return;
    }
    let source = context.source;
    let redirects = redirects_of(node);
    let [first, second] = redirects.as_slice() else {
        return;
    };
    if is_stderr_to_stdout(*first, source) && is_output_to_file(*second, source) && !is_captured(node) {
        findings.report(ShellRule::Sc2069, *first, text(*first, source));
    }
}

fn redirects_of(node: Node<'_>) -> Vec<Node<'_>> {
    let mut redirects: Vec<Node<'_>> = children(node)
        .into_iter()
        .filter(|child| REDIRECT_KINDS.contains(&child.kind()))
        .collect();
    if node.kind() == REDIRECTED
        && let Some(body) = node
            .child_by_field_name(BODY_FIELD)
            .filter(|body| body.kind() == COMMAND)
    {
        redirects.extend(
            children(body)
                .into_iter()
                .filter(|child| REDIRECT_KINDS.contains(&child.kind())),
        );
    }
    redirects.sort_by_key(Node::start_byte);
    redirects
}

fn is_stderr_to_stdout(redirect: Node<'_>, source: &[u8]) -> bool {
    redirect.kind() == FILE_REDIRECT
        && descriptor(redirect, source) == Some(STDERR)
        && operator(redirect, source) == Some(DUPLICATE_OUTPUT)
        && redirect
            .child_by_field_name(DESTINATION_FIELD)
            .is_some_and(|destination| text(destination, source) == STDOUT)
}

fn is_output_to_file(redirect: Node<'_>, source: &[u8]) -> bool {
    redirect.kind() == FILE_REDIRECT
        && operator(redirect, source).is_some_and(|operator| FILE_OUTPUTS.contains(&operator))
}

fn descriptor<'source>(redirect: Node<'_>, source: &'source [u8]) -> Option<&'source str> {
    redirect
        .child_by_field_name(DESCRIPTOR_FIELD)
        .map(|descriptor| text(descriptor, source))
}

fn operator<'source>(redirect: Node<'_>, source: &'source [u8]) -> Option<&'source str> {
    children(redirect)
        .into_iter()
        .find(|child| !child.is_named())
        .map(|operator| text(operator, source))
}

fn is_captured(node: Node<'_>) -> bool {
    let mut child = node;
    while let Some(parent) = child.parent() {
        let captured = match parent.kind() {
            PIPELINE => crate::shell::syntax::named_children(parent).last() != Some(&child),
            COMMAND_SUBSTITUTION | PROCESS_SUBSTITUTION => true,
            _ => false,
        };
        if captured {
            return true;
        }
        child = parent;
    }
    false
}
