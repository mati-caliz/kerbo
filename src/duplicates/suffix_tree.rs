use std::collections::BTreeMap;

const INFINITY: i64 = 2_147_483_647;
const NO_LEFT_CONTEXT: i32 = 0;
const INT32_SPAN: i64 = 4_294_967_296;
const INT32_HALF: i64 = 2_147_483_648;
const ROOT: usize = 0;
const AUXILIARY: usize = 1;

#[derive(Debug, Clone, Copy)]
struct Transition {
    start: i64,
    end: i64,
    state: usize,
}

#[derive(Debug, Default)]
struct State {
    transitions: Vec<usize>,
    link: usize,
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
        })
    }

    fn find_transition(&self, state: usize, value: i32) -> Option<usize> {
        self.states
            .get(state)?
            .transitions
            .iter()
            .copied()
            .find(|&index| {
                let transition = self.transition(index);
                self.value(transition.start) == value
            })
    }

    fn new_state(&mut self) -> usize {
        self.states.push(State::default());
        self.states.len().saturating_sub(1)
    }

    fn add_transition(&mut self, from: usize, transition: Transition) {
        self.transitions.push(transition);
        let index = self.transitions.len().saturating_sub(1);
        if let Some(state) = self.states.get_mut(from) {
            state.transitions.push(index);
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

    pub fn repeats_over(&self, threshold: usize) -> Vec<Repeat> {
        let threshold = i64::try_from(threshold).unwrap_or(i64::MAX);
        let mut found = Vec::new();
        let root = Frame::new(
            Transition {
                start: 0,
                end: 0,
                state: ROOT,
            },
            0,
        );
        let mut stack = vec![root];
        while let Some(frame) = stack.last_mut() {
            let next = self
                .states
                .get(frame.transition.state)
                .and_then(|state| state.transitions.get(frame.next_child))
                .copied();
            if let Some(index) = next {
                frame.next_child = frame.next_child.saturating_add(1);
                let child = self.transition(index);
                let length = frame.length.saturating_add(go_length(child));
                stack.push(Frame::new(child, length));
                continue;
            }
            let Some(mut finished) = stack.pop() else {
                break;
            };
            if self
                .states
                .get(finished.transition.state)
                .is_none_or(|state| state.transitions.is_empty())
            {
                finished.contexts = self.leaf_context(finished.transition, finished.length);
            } else if finished.length >= threshold && finished.contexts.len() > 1 {
                found.push(Repeat {
                    positions: finished.contexts.values().flatten().copied().collect(),
                    length: usize::try_from(finished.length).unwrap_or_default(),
                });
            }
            if let Some(parent) = stack.last_mut().filter(|_| finished.length >= threshold) {
                parent.absorb(finished.contexts);
            }
        }
        found
    }

    fn leaf_context(&self, transition: Transition, length: i64) -> BTreeMap<i32, Vec<usize>> {
        let start =
            wrap_int32(wrap_int32(transition.end.saturating_add(1)).saturating_sub(wrap_int32(length)));
        let context = if start > 0 {
            self.value(start.saturating_sub(1))
        } else {
            NO_LEFT_CONTEXT
        };
        BTreeMap::from([(context, usize::try_from(start).into_iter().collect())])
    }
}

#[derive(Debug)]
struct Frame {
    transition: Transition,
    length: i64,
    next_child: usize,
    contexts: BTreeMap<i32, Vec<usize>>,
}

impl Frame {
    const fn new(transition: Transition, length: i64) -> Self {
        Self {
            transition,
            length,
            next_child: 0,
            contexts: BTreeMap::new(),
        }
    }

    fn absorb(&mut self, contexts: BTreeMap<i32, Vec<usize>>) {
        for (context, positions) in contexts {
            self.contexts.entry(context).or_default().extend(positions);
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
