use std::collections::HashSet;

use tree_sitter::Node;

use super::NestingFinding;
use crate::macro_rules::{Code, Expansion, MacroScope};
use crate::metrics::clippy::cfg::is_compiled_out;
use crate::metrics::clippy::macros::arguments_outside_blocks;
use crate::syntax::one_based;

const BLOCK: &str = "block";
const UNSAFE_BLOCK: &str = "unsafe_block";
const NESTING_ITEMS: &[&str] = &["impl_item", "trait_item"];
const MODULE: &str = "mod_item";
const MACRO: &str = "macro_invocation";
const BODY_FIELD: &str = "body";
const ITEM_SUFFIX: &str = "_item";

#[derive(Debug)]
struct Findings {
    limit: usize,
    found: Vec<NestingFinding>,
}

// Calca excessive_nesting de clippy: suma un nivel por cada bloque entre llaves y por cada impl,
// trait o módulo en línea, marca el primero que pasa el límite y no sigue bajando por él. Lo que
// viene de una `macro_rules!` del crate se expande, pero clippy deja de bajar en el primer bloque o
// ítem que arma la macro: sólo cuentan los bloques escritos en la invocación que la expansión deja
// a la vista. Un bloque `unsafe` no se mira: su span arranca en `unsafe` y no en la llave, y clippy
// lo saltea entero.
pub(super) fn reported(root: Node<'_>, limit: usize, scope: &MacroScope<'_>) -> Vec<NestingFinding> {
    let mut findings = Findings {
        limit,
        found: Vec::new(),
    };
    walk(root, &Code::root(scope.current_text(), scope), 0, &mut findings);
    let mut seen = HashSet::new();
    findings
        .found
        .retain(|finding| seen.insert((finding.line, finding.column)));
    findings.found
}

fn walk(root: Node<'_>, code: &Code<'_>, start_level: usize, findings: &mut Findings) {
    let mut pending = vec![(root, start_level)];
    while let Some((node, outer_level)) = pending.pop() {
        let kind = node.kind();
        if kind == UNSAFE_BLOCK
            || (is_spanned(kind) && !is_written_here(node, code))
            || is_compiled_out(node, code)
        {
            continue;
        }
        if kind == MACRO {
            match code.expand_node(node) {
                Some(expansion) => walk_expansion(&expansion, code, outer_level, findings),
                None => walk_arguments(node, code, outer_level, findings),
            }
            continue;
        }
        let nests = adds_level(node);
        let level = if nests {
            outer_level.saturating_add(1)
        } else {
            outer_level
        };
        if nests && level > findings.limit {
            findings.found.extend(position(node, code, level));
            continue;
        }
        pending.extend(children(node).into_iter().rev().map(|child| (child, level)));
    }
}

fn walk_expansion(expansion: &Expansion, code: &Code<'_>, level: usize, findings: &mut Findings) {
    let expanded = expansion.code(code.scope());
    let Some(content) = expansion.content() else {
        return;
    };
    for child in children(content) {
        walk(child, &expanded, level, findings);
    }
}

fn walk_arguments(invocation: Node<'_>, code: &Code<'_>, level: usize, findings: &mut Findings) {
    for argument in arguments_outside_blocks(invocation, code) {
        if let Some(expression) = argument.expression() {
            walk(expression, &argument.code(code), level, findings);
        }
    }
}

fn children(node: Node<'_>) -> Vec<Node<'_>> {
    (0..node.child_count())
        .filter_map(|index| node.child(index))
        .collect()
}

fn is_spanned(kind: &str) -> bool {
    kind == BLOCK || kind.ends_with(ITEM_SUFFIX)
}

fn is_written_here(node: Node<'_>, code: &Code<'_>) -> bool {
    node.end_byte()
        .checked_sub(1)
        .is_some_and(|last| code.is_written_here(node.start_byte(), last))
}

fn position(node: Node<'_>, code: &Code<'_>, depth: usize) -> Option<NestingFinding> {
    let (row, column) = code.position(node.start_byte())?;
    Some(NestingFinding {
        line: one_based(row),
        column: one_based(column),
        depth,
    })
}

fn adds_level(node: Node<'_>) -> bool {
    match node.kind() {
        BLOCK => true,
        MODULE => node.child_by_field_name(BODY_FIELD).is_some(),
        kind => NESTING_ITEMS.contains(&kind),
    }
}
