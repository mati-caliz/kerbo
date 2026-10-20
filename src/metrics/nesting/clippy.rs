use tree_sitter::Node;

use super::{NestingFinding, position};

const BLOCK: &str = "block";
const UNSAFE_BLOCK: &str = "unsafe_block";
const NESTING_ITEMS: &[&str] = &["impl_item", "trait_item"];
const MODULE: &str = "mod_item";
const BODY_FIELD: &str = "body";

// Calca excessive_nesting de clippy: suma un nivel por cada bloque entre llaves y por cada impl,
// trait o módulo en línea, marca el primero que pasa el límite y no sigue bajando por él. Lo que
// está adentro de una macro no se mira: clippy sólo ve lo que la expansión deja con su span. Un
// bloque `unsafe` tampoco: su span arranca en `unsafe` y no en la llave, y clippy lo saltea entero.
pub(super) fn reported(root: Node<'_>, limit: usize) -> Vec<NestingFinding> {
    let mut findings = Vec::new();
    let mut pending = vec![(root, 0_usize)];
    while let Some((node, outer_level)) = pending.pop() {
        if node.kind() == UNSAFE_BLOCK {
            continue;
        }
        let nests = adds_level(node);
        let level = if nests {
            outer_level.saturating_add(1)
        } else {
            outer_level
        };
        if nests && level > limit {
            findings.push(position(node, level));
            continue;
        }
        pending.extend(
            (0..node.child_count())
                .rev()
                .filter_map(|index| node.child(index))
                .map(|child| (child, level)),
        );
    }
    findings
}

fn adds_level(node: Node<'_>) -> bool {
    match node.kind() {
        BLOCK => true,
        MODULE => node.child_by_field_name(BODY_FIELD).is_some(),
        kind => NESTING_ITEMS.contains(&kind),
    }
}
