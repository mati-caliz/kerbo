const INFINITY: i64 = 2_147_483_647;
const NO_LEFT_CONTEXT: i32 = 0;
const INT32_SPAN: i64 = 4_294_967_296;
const INT32_HALF: i64 = 2_147_483_648;
const ROOT: usize = 0;
const AUXILIARY: usize = 1;
const NONE: usize = usize::MAX;

#[derive(Debug, Clone, Copy)]
struct Transition {
    start: i64,
    end: i64,
    state: usize,
    next: usize,
}

#[derive(Debug, Clone, Copy)]
struct State {
    first: usize,
    last: usize,
    link: usize,
}

impl Default for State {
    fn default() -> Self {
        Self {
            first: NONE,
            last: NONE,
            link: ROOT,
        }
    }
}

#[derive(Debug)]
pub struct Repeat {
    pub positions: Vec<usize>,
    pub length: usize,
}

// Calca el árbol de sufijos de dupl (Ukkonen) con su mismo orden de transiciones: el orden en que
// aparecen las posiciones de una repetición decide cuál se toma como primera copia.
#[derive(Debug)]
pub struct SuffixTree {
    data: Vec<i32>,
    states: Vec<State>,
    transitions: Vec<Transition>,
    active: usize,
    start: i64,
    end: i64,
}

impl SuffixTree {
    pub fn new() -> Self {
        let mut tree = Self {
            data: Vec::new(),
            states: vec![State::default(), State::default()],
            transitions: Vec::new(),
            active: ROOT,
            start: 0,
            end: 0,
        };
        if let Some(root) = tree.states.get_mut(ROOT) {
            root.link = AUXILIARY;
        }
        tree
    }

    pub fn push(&mut self, value: i32) {
        self.data.push(value);
        self.update();
        let (state, start) = self.canonize(self.active, self.start, self.end);
        self.active = state;
        self.start = start;
        self.end = self.end.saturating_add(1);
    }

    fn value(&self, position: i64) -> i32 {
        usize::try_from(position)
            .ok()
            .and_then(|index| self.data.get(index).copied())
            .unwrap_or_default()
    }

    fn transition(&self, index: usize) -> Transition {
        self.transitions.get(index).copied().unwrap_or(Transition {
            start: 0,
            end: 0,
            state: ROOT,
            next: NONE,
        })
    }

    fn transitions_of(&self, state: usize) -> impl Iterator<Item = usize> + '_ {
        let first = self.states.get(state).map_or(NONE, |found| found.first);
        std::iter::successors(Some(first).filter(|&index| index != NONE), |&index| {
            Some(self.transition(index).next).filter(|&next| next != NONE)
        })
    }

    fn find_transition(&self, state: usize, value: i32) -> Option<usize> {
        self.transitions_of(state)
            .find(|&index| self.value(self.transition(index).start) == value)
    }

    fn new_state(&mut self) -> usize {
        self.states.push(State::default());
        self.states.len().saturating_sub(1)
    }

    fn add_transition(&mut self, from: usize, transition: Transition) {
        self.transitions.push(transition);
        let index = self.transitions.len().saturating_sub(1);
        let Some(state) = self.states.get_mut(from) else {
            return;
        };
        let previous_last = state.last;
        state.last = index;
        if previous_last == NONE {
            state.first = index;
        } else if let Some(previous) = self.transitions.get_mut(previous_last) {
            previous.next = index;
        }
    }

    fn set_link(&mut self, state: usize, link: usize) {
        if let Some(found) = self.states.get_mut(state) {
            found.link = link;
        }
    }

    fn link(&self, state: usize) -> usize {
        self.states.get(state).map_or(ROOT, |found| found.link)
    }

    fn update(&mut self) {
        let mut previous = ROOT;
        let mut state = self.active;
        let mut start = self.start;
        let end = self.end;
        let reached = loop {
            let (split, is_end_point) = self.test_and_split(state, start, end.saturating_sub(1));
            if is_end_point {
                break split;
            }
            let leaf = self.new_state();
            self.add_transition(
                split,
                Transition {
                    start: end,
                    end: INFINITY,
                    state: leaf,
                    next: NONE,
                },
            );
            if previous != ROOT {
                self.set_link(previous, split);
            }
            previous = split;
            (state, start) = self.canonize(self.link(state), start, end.saturating_sub(1));
        };
        if previous != ROOT {
            self.set_link(previous, reached);
        }
        self.active = state;
        self.start = start;
    }

    fn test_and_split(&mut self, state: usize, start: i64, end: i64) -> (usize, bool) {
        let current = self.value(self.end);
        if start <= end {
            let Some(index) = self.find_transition(state, self.value(start)) else {
                return (state, true);
            };
            let transition = self.transition(index);
            let split_point = transition
                .start
                .saturating_add(end)
                .saturating_sub(start)
                .saturating_add(1);
            if self.value(split_point) == current {
                return (state, true);
            }
            let middle = self.new_state();
            self.add_transition(
                middle,
                Transition {
                    start: split_point,
                    end: transition.end,
                    state: transition.state,
                    next: NONE,
                },
            );
            if let Some(shortened) = self.transitions.get_mut(index) {
                shortened.end = split_point.saturating_sub(1);
                shortened.state = middle;
            }
            return (middle, false);
        }
        let is_end_point = state == AUXILIARY || self.find_transition(state, current).is_some();
        (state, is_end_point)
    }

    fn canonize(&self, state: usize, start: i64, end: i64) -> (usize, i64) {
        let (mut state, mut start) = if state == AUXILIARY {
            (ROOT, start.saturating_add(1))
        } else {
            (state, start)
        };
        if start > end {
            return (state, start);
        }
        let mut transition = self.transition(0);
        loop {
            if start <= end {
                match self.find_transition(state, self.value(start)) {
                    Some(index) => transition = self.transition(index),
                    None => return (state, start),
                }
            }
            let width = transition.end.saturating_sub(transition.start);
            if width > end.saturating_sub(start) {
                return (state, start);
            }
            start = start.saturating_add(width).saturating_add(1);
            state = transition.state;
        }
    }

    // Las posiciones de un nodo son las hojas de su subárbol: en un recorrido en profundidad quedan
    // contiguas, así que cada nodo guarda sólo dónde empiezan las suyas en vez de copiar listas
    // hacia arriba (con datos repetitivos, como las tablas generadas, eso era cuadrático).
    pub fn for_each_repeat(&self, threshold: usize, mut visit: impl FnMut(&Repeat)) {
        let threshold = i64::try_from(threshold).unwrap_or(i64::MAX);
        let mut leaves: Vec<(i32, usize)> = Vec::new();
        let mut stack = vec![Frame::new(0, 0, self.first_transition(ROOT))];
        while let Some(frame) = stack.last_mut() {
            if frame.next_child != NONE {
                let child = self.transition(frame.next_child);
                frame.next_child = child.next;
                let length = frame.length.saturating_add(go_length(child));
                stack.extend(self.descend(child, length, &mut leaves));
                continue;
            }
            let Some(finished) = stack.pop() else {
                break;
            };
            if finished.length >= threshold
                && let Some(found) = repeat(
                    leaves.get(finished.first_leaf..).unwrap_or_default(),
                    finished.length,
                )
            {
                visit(&found);
            }
        }
    }

    fn descend(&self, child: Transition, length: i64, leaves: &mut Vec<(i32, usize)>) -> Option<Frame> {
        let first_grandchild = self.first_transition(child.state);
        if first_grandchild == NONE {
            leaves.extend(self.leaf_context(child, length));
            return None;
        }
        Some(Frame::new(length, leaves.len(), first_grandchild))
    }

    fn first_transition(&self, state: usize) -> usize {
        self.states.get(state).map_or(NONE, |found| found.first)
    }

    fn leaf_context(&self, transition: Transition, length: i64) -> Option<(i32, usize)> {
        let start =
            wrap_int32(wrap_int32(transition.end.saturating_add(1)).saturating_sub(wrap_int32(length)));
        let context = if start > 0 {
            self.value(start.saturating_sub(1))
        } else {
            NO_LEFT_CONTEXT
        };
        usize::try_from(start).ok().map(|position| (context, position))
    }
}

fn repeat(leaves: &[(i32, usize)], length: i64) -> Option<Repeat> {
    let first_context = leaves.first()?.0;
    if leaves.iter().all(|&(context, _)| context == first_context) {
        return None;
    }
    let mut ordered = leaves.to_vec();
    ordered.sort_by_key(|&(context, _)| context);
    Some(Repeat {
        positions: ordered.into_iter().map(|(_, position)| position).collect(),
        length: usize::try_from(length).unwrap_or_default(),
    })
}

#[derive(Debug)]
struct Frame {
    length: i64,
    first_leaf: usize,
    next_child: usize,
}

impl Frame {
    const fn new(length: i64, first_leaf: usize, next_child: usize) -> Self {
        Self {
            length,
            first_leaf,
            next_child,
        }
    }
}

// dupl guarda las posiciones en int32 y deja que el largo de una hoja (que termina en "infinito")
// desborde: la cuenta envuelta da justo el comienzo del sufijo. Se reproduce el desborde tal cual.
const fn go_length(transition: Transition) -> i64 {
    wrap_int32(transition.end.saturating_sub(transition.start).saturating_add(1))
}

const fn wrap_int32(value: i64) -> i64 {
    (value.saturating_add(INT32_HALF))
        .rem_euclid(INT32_SPAN)
        .saturating_sub(INT32_HALF)
}
