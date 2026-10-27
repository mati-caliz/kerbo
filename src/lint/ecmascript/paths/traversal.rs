use std::collections::HashSet;

use super::segments::{Graph, SegmentId};

// El `traverseSegments` de ESLint: desde `first`, cada segmento una vez y sólo después de todos sus
// anteriores (salvo los que vuelven por un loop). Si `visit` devuelve `true` se saltean los que
// sólo se alcanzan pasando por ese segmento; `last` se saltea siempre.
pub fn traverse_segments(
    graph: &Graph,
    first: SegmentId,
    last: Option<SegmentId>,
    mut visit: impl FnMut(SegmentId) -> bool,
) {
    let mut visited: HashSet<SegmentId> = HashSet::new();
    let mut skipped: HashSet<SegmentId> = HashSet::new();
    let mut stack: Vec<(SegmentId, usize)> = vec![(first, 0)];
    while let Some(&(segment, index)) = stack.last() {
        if index == 0 {
            if visited.contains(&segment) {
                stack.pop();
                continue;
            }
            let previous = graph.previous(segment);
            let ready = previous
                .iter()
                .all(|prior| visited.contains(prior) || graph.is_looped_previous(segment, *prior));
            if segment != first && !previous.is_empty() && !ready {
                stack.pop();
                continue;
            }
            visited.insert(segment);
            let should_skip = !skipped.is_empty()
                && !previous.is_empty()
                && previous
                    .iter()
                    .all(|prior| skipped.contains(prior) || graph.is_looped_previous(segment, *prior));
            if should_skip || visit(segment) || last == Some(segment) {
                skipped.insert(segment);
            }
        }
        let next = graph.next(segment);
        let Some(top) = stack.last_mut() else {
            break;
        };
        match next.get(index) {
            Some(following) if index.saturating_add(1) < next.len() => {
                top.1 = index.saturating_add(1);
                stack.push((*following, 0));
            }
            Some(following) => *top = (*following, 0),
            None => {
                stack.pop();
            }
        }
    }
}
