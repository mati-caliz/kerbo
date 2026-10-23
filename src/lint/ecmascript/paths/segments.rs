pub type SegmentId = usize;
pub type ForkId = usize;

const LEAVING_PATH_FACTOR: usize = 2;
const FIRST: isize = 0;
const LAST: isize = -1;
pub const WHOLE: (isize, isize) = (FIRST, LAST);
pub const HEAD: (isize, isize) = (LAST, LAST);

#[derive(Debug, Default)]
pub struct Segment {
    pub reachable: bool,
    pub next: Vec<SegmentId>,
    pub previous: Vec<SegmentId>,
    pub all_next: Vec<SegmentId>,
    pub all_previous: Vec<SegmentId>,
    pub looped_previous: Vec<SegmentId>,
    pub used: bool,
}

// `ForkContext` de ESLint: una pila de listas de segmentos paralelos (`count` caminos a la vez,
// el doble adentro de un `finally`, que separa el camino normal del que sale por return o throw).
#[derive(Debug)]
struct Fork {
    upper: Option<ForkId>,
    count: usize,
    list: Vec<Vec<SegmentId>>,
}

#[derive(Clone, Copy)]
enum Creation {
    Next,
    Unreachable,
    Disconnected,
}

// Los segmentos de todos los caminos de un archivo y sus contextos de bifurcación.
#[derive(Debug, Default)]
pub struct Graph {
    segments: Vec<Segment>,
    forks: Vec<Fork>,
}

impl Graph {
    pub fn is_reachable(&self, segment: SegmentId) -> bool {
        self.segments.get(segment).is_some_and(|found| found.reachable)
    }

    pub fn any_reachable(&self, segments: &[SegmentId]) -> bool {
        segments.iter().any(|segment| self.is_reachable(*segment))
    }

    fn push_segment(&mut self, all_previous: Vec<SegmentId>, reachable: bool) -> SegmentId {
        let previous = all_previous
            .iter()
            .copied()
            .filter(|segment| self.is_reachable(*segment))
            .collect();
        self.segments.push(Segment {
            reachable,
            previous,
            all_previous,
            ..Segment::default()
        });
        self.segments.len().saturating_sub(1)
    }

    pub fn new_next(&mut self, all_previous: &[SegmentId]) -> SegmentId {
        let reachable = self.any_reachable(all_previous);
        let flattened = self.flatten_unused(all_previous);
        self.push_segment(flattened, reachable)
    }

    fn new_unreachable(&mut self, all_previous: &[SegmentId]) -> SegmentId {
        let flattened = self.flatten_unused(all_previous);
        let segment = self.push_segment(flattened, false);
        self.mark_used(segment);
        segment
    }

    fn new_disconnected(&mut self, all_previous: &[SegmentId]) -> SegmentId {
        let reachable = self.any_reachable(all_previous);
        self.push_segment(Vec::new(), reachable)
    }

    pub fn mark_used(&mut self, segment: SegmentId) {
        let Some(found) = self.segments.get_mut(segment).filter(|found| !found.used) else {
            return;
        };
        found.used = true;
        let reachable = found.reachable;
        let previous = found.all_previous.clone();
        for prior in previous {
            self.link_forward(prior, segment, reachable);
        }
    }

    fn link_forward(&mut self, prior: SegmentId, segment: SegmentId, reachable: bool) {
        let Some(found) = self.segments.get_mut(prior) else {
            return;
        };
        found.all_next.push(segment);
        if reachable {
            found.next.push(segment);
        }
    }

    // `flattenUnusedSegments`: un segmento que nunca se usó se reemplaza por sus anteriores.
    fn flatten_unused(&self, segments: &[SegmentId]) -> Vec<SegmentId> {
        let mut done: Vec<SegmentId> = Vec::new();
        for segment in segments {
            if done.contains(segment) {
                continue;
            }
            let replacement = match self.segments.get(*segment) {
                Some(found) if !found.used => found.all_previous.clone(),
                _ => vec![*segment],
            };
            for candidate in replacement {
                push_unique(&mut done, candidate);
            }
        }
        done
    }

    // `makeLooped`: une el final de una vuelta con el principio de la siguiente.
    pub fn make_looped(&mut self, from_segments: &[SegmentId], to_segments: &[SegmentId]) {
        let sources = self.flatten_unused(from_segments);
        let targets = self.flatten_unused(to_segments);
        for (source, target) in sources.into_iter().zip(targets) {
            self.link_loop(source, target);
        }
    }

    fn link_loop(&mut self, source: SegmentId, target: SegmentId) {
        let target_reachable = self.is_reachable(target);
        let source_reachable = self.is_reachable(source);
        if let Some(found) = self.segments.get_mut(source) {
            if target_reachable {
                found.next.push(target);
            }
            found.all_next.push(target);
        }
        let Some(found) = self.segments.get_mut(target) else {
            return;
        };
        if source_reachable {
            found.previous.push(source);
        }
        found.all_previous.push(source);
        if found.all_previous.len() >= LEAVING_PATH_FACTOR {
            found.looped_previous.push(source);
        }
    }

    // `disconnectSegments`: corta el paso directo de cada anterior a su siguiente. Como el
    // `splice(indexOf(...), 1)` de ESLint, si el segmento no está se borra el último.
    pub fn disconnect(&mut self, previous: &[SegmentId], next: &[SegmentId]) {
        for (prior, following) in previous.iter().zip(next) {
            if let Some(found) = self.segments.get_mut(*prior) {
                remove_first(&mut found.next, *following);
                remove_first(&mut found.all_next, *following);
            }
            if let Some(found) = self.segments.get_mut(*following) {
                remove_first(&mut found.previous, *prior);
                remove_first(&mut found.all_previous, *prior);
            }
        }
    }

    pub fn new_root_fork(&mut self) -> ForkId {
        let root = self.push_segment(Vec::new(), true);
        self.forks.push(Fork {
            upper: None,
            count: 1,
            list: vec![vec![root]],
        });
        self.forks.len().saturating_sub(1)
    }

    pub fn new_empty_fork(&mut self, parent: ForkId, forks_leaving_path: bool) -> ForkId {
        let factor = if forks_leaving_path {
            LEAVING_PATH_FACTOR
        } else {
            1
        };
        let count = self.count(parent).saturating_mul(factor);
        self.forks.push(Fork {
            upper: Some(parent),
            count,
            list: Vec::new(),
        });
        self.forks.len().saturating_sub(1)
    }

    pub fn successor_lists(&self) -> Vec<Vec<SegmentId>> {
        self.segments.iter().map(|segment| segment.next.clone()).collect()
    }

    pub fn upper(&self, fork: ForkId) -> Option<ForkId> {
        self.forks.get(fork).and_then(|found| found.upper)
    }

    pub fn count(&self, fork: ForkId) -> usize {
        self.forks.get(fork).map_or(1, |found| found.count)
    }

    pub fn head(&self, fork: ForkId) -> Vec<SegmentId> {
        self.forks
            .get(fork)
            .and_then(|found| found.list.last().cloned())
            .unwrap_or_default()
    }

    pub fn is_empty(&self, fork: ForkId) -> bool {
        self.forks.get(fork).is_none_or(|found| found.list.is_empty())
    }

    pub fn is_fork_reachable(&self, fork: ForkId) -> bool {
        let head = self.head(fork);
        !head.is_empty() && self.any_reachable(&head)
    }

    pub fn segments_list(&self, fork: ForkId) -> Vec<Vec<SegmentId>> {
        self.forks
            .get(fork)
            .map(|found| found.list.clone())
            .unwrap_or_default()
    }

    pub fn make_next(&mut self, fork: ForkId, range: (isize, isize)) -> Vec<SegmentId> {
        self.create_segments(fork, range, Creation::Next)
    }

    pub fn make_unreachable(&mut self, fork: ForkId, range: (isize, isize)) -> Vec<SegmentId> {
        self.create_segments(fork, range, Creation::Unreachable)
    }

    pub fn make_disconnected(&mut self, fork: ForkId, range: (isize, isize)) -> Vec<SegmentId> {
        self.create_segments(fork, range, Creation::Disconnected)
    }

    fn create_segments(
        &mut self,
        fork: ForkId,
        (start, end): (isize, isize),
        creation: Creation,
    ) -> Vec<SegmentId> {
        let list = self.segments_list(fork);
        let begin = normalized_index(start, list.len());
        let finish = normalized_index(end, list.len());
        let selected = list.get(begin..=finish).unwrap_or_default();
        (0..self.count(fork))
            .map(|path| {
                let previous: Vec<SegmentId> = selected
                    .iter()
                    .filter_map(|segments| segments.get(path).copied())
                    .collect();
                match creation {
                    Creation::Next => self.new_next(&previous),
                    Creation::Unreachable => self.new_unreachable(&previous),
                    Creation::Disconnected => self.new_disconnected(&previous),
                }
            })
            .collect()
    }

    // `mergeExtraSegments`: si llegan más caminos que los del contexto, se juntan de a pares.
    fn merge_extra(&mut self, fork: ForkId, segments: Vec<SegmentId>) -> Vec<SegmentId> {
        let count = self.count(fork);
        let mut current = segments;
        while current.len() > count {
            let half = current.len() / LEAVING_PATH_FACTOR;
            let pairs: Vec<Vec<SegmentId>> = (0..half)
                .map(|index| {
                    [current.get(index), current.get(index.saturating_add(half))]
                        .into_iter()
                        .flatten()
                        .copied()
                        .collect()
                })
                .collect();
            current = pairs.iter().map(|pair| self.new_next(pair)).collect();
        }
        current
    }

    pub fn add(&mut self, fork: ForkId, segments: Vec<SegmentId>) {
        let merged = self.merge_extra(fork, segments);
        if let Some(found) = self.forks.get_mut(fork) {
            found.list.push(merged);
        }
    }

    pub fn replace_head(&mut self, fork: ForkId, segments: Vec<SegmentId>) {
        let merged = self.merge_extra(fork, segments);
        if let Some(found) = self.forks.get_mut(fork) {
            found.list.pop();
            found.list.push(merged);
        }
    }

    pub fn add_all(&mut self, fork: ForkId, other: ForkId) {
        let others = self.segments_list(other);
        if let Some(found) = self.forks.get_mut(fork) {
            found.list.extend(others);
        }
    }

    pub fn clear(&mut self, fork: ForkId) {
        if let Some(found) = self.forks.get_mut(fork) {
            found.list.clear();
        }
    }
}

const fn normalized_index(index: isize, length: usize) -> usize {
    if index >= 0 {
        index.unsigned_abs()
    } else {
        length.saturating_sub(index.unsigned_abs())
    }
}

fn push_unique(list: &mut Vec<SegmentId>, value: SegmentId) {
    if !list.contains(&value) {
        list.push(value);
    }
}

fn remove_first(list: &mut Vec<SegmentId>, value: SegmentId) {
    match list.iter().position(|candidate| *candidate == value) {
        Some(position) => {
            list.remove(position);
        }
        None => {
            list.pop();
        }
    }
}
