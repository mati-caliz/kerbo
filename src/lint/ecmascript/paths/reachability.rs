use std::collections::HashMap;

use tree_sitter::Node;

use super::roles::is_identifier;
use super::segments::SegmentId;
use super::supers::DerivedConstructor;

const TRY: &str = "try_statement";
const BODY_FIELD: &str = "body";
const DECLARATOR: &str = "variable_declarator";
const VALUE_FIELD: &str = "value";
const ASSIGNMENTS: &[&str] = &[
    "assignment_expression",
    "augmented_assignment_expression",
    "update_expression",
];

// El primer y el último identificador que se vieron mientras el segmento estaba activo.
#[derive(Debug, Clone, Copy)]
pub struct SegmentSpan<'tree> {
    pub first: Node<'tree>,
    pub last: Node<'tree>,
}

// Una asignación (`x = 1`, `let x = 1`, `x++`) con los segmentos activos al terminarla.
#[derive(Debug)]
pub struct Assignment<'tree> {
    pub node: Node<'tree>,
    pub segments: Vec<SegmentId>,
}

// Lo que las reglas de ESLint juntan de un camino escuchando sus eventos: los segmentos alcanzables
// activos, lo que abarca cada uno, los bloques `try` y las asignaciones.
#[derive(Debug)]
pub struct PathRecord<'tree> {
    pub start: Node<'tree>,
    pub spans: HashMap<SegmentId, Option<SegmentSpan<'tree>>>,
    pub current: Vec<SegmentId>,
    pub try_blocks: Vec<Node<'tree>>,
    pub assignments: Vec<Assignment<'tree>>,
    pub initial: Option<SegmentId>,
    pub derived: Option<DerivedConstructor<'tree>>,
}

impl<'tree> PathRecord<'tree> {
    pub fn new(start: Node<'tree>) -> Self {
        Self {
            start,
            spans: HashMap::new(),
            current: Vec::new(),
            try_blocks: Vec::new(),
            assignments: Vec::new(),
            initial: None,
            derived: None,
        }
    }

    pub fn end_segment(&mut self, segment: SegmentId) {
        self.current.retain(|current| *current != segment);
    }

    pub fn start_segment(&mut self, segment: SegmentId) {
        self.spans.insert(segment, None);
        if !self.current.contains(&segment) {
            self.current.push(segment);
        }
    }

    fn see_identifier(&mut self, identifier: Node<'tree>) {
        for segment in &self.current {
            let span = self.spans.entry(*segment).or_insert(None);
            *span = Some(span.map_or(
                SegmentSpan {
                    first: identifier,
                    last: identifier,
                },
                |known| SegmentSpan {
                    first: known.first,
                    last: identifier,
                },
            ));
        }
    }

    // Lo que escuchan `no-useless-assignment` y compañía al entrar a un nodo.
    pub fn observe_enter(&mut self, node: Node<'tree>) {
        if is_identifier(node.kind()) {
            self.see_identifier(node);
        } else if let Some(block) = node
            .child_by_field_name(BODY_FIELD)
            .filter(|_| node.kind() == TRY)
        {
            self.try_blocks.push(block);
        }
    }

    pub fn observe_exit(&mut self, node: Node<'tree>) {
        let is_assignment = ASSIGNMENTS.contains(&node.kind())
            || (node.kind() == DECLARATOR && node.child_by_field_name(VALUE_FIELD).is_some());
        if is_assignment {
            self.see_assignment(node);
        }
    }

    fn see_assignment(&mut self, node: Node<'tree>) {
        if !self.current.is_empty() {
            self.assignments.push(Assignment {
                node,
                segments: self.current.clone(),
            });
        }
    }
}

// Todo lo que deja el análisis de caminos de un archivo.
#[derive(Debug, Default)]
pub struct Flow<'tree> {
    pub at_enter: HashMap<usize, bool>,
    pub at_exit: HashMap<usize, bool>,
    pub paths: Vec<PathRecord<'tree>>,
    pub successors: Vec<Vec<SegmentId>>,
}

impl Flow<'_> {
    pub fn reachable_at_enter(&self, node: Node<'_>) -> bool {
        self.at_enter.get(&node.id()).copied().unwrap_or(true)
    }

    pub fn reachable_at_exit(&self, node: Node<'_>) -> bool {
        self.at_exit.get(&node.id()).copied().unwrap_or(true)
    }

    pub fn successors(&self, segment: SegmentId) -> &[SegmentId] {
        self.successors.get(segment).map_or(&[], Vec::as_slice)
    }
}

// Qué segmentos terminan y cuáles empiezan cuando los actuales pasan a ser la cabeza: los que
// cambian en la misma posición, como compara `forwardCurrentToHead`.
pub fn segment_changes(current: &[SegmentId], head: &[SegmentId]) -> (Vec<SegmentId>, Vec<SegmentId>) {
    let changed = |from: &[SegmentId], to: &[SegmentId]| -> Vec<SegmentId> {
        from.iter()
            .enumerate()
            .filter(|(index, segment)| to.get(*index) != Some(*segment))
            .map(|(_, segment)| *segment)
            .collect()
    };
    (changed(current, head), changed(head, current))
}
