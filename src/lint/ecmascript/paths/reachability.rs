use std::collections::HashMap;

use tree_sitter::Node;

// Si algún segmento actual es alcanzable al entrar a cada nodo y al salir de él, como lo ven las
// reglas de ESLint que escuchan `onCodePathSegmentStart` y `onCodePathSegmentEnd`.
#[derive(Debug, Default)]
pub struct Reachability {
    pub at_enter: HashMap<usize, bool>,
    pub at_exit: HashMap<usize, bool>,
}

impl Reachability {
    pub fn reachable_at_enter(&self, node: Node<'_>) -> bool {
        self.at_enter.get(&node.id()).copied().unwrap_or(true)
    }

    pub fn reachable_at_exit(&self, node: Node<'_>) -> bool {
        self.at_exit.get(&node.id()).copied().unwrap_or(true)
    }
}
