pub use super::contexts::{Break, Choice, ChoiceKind, Loop, LoopKind, Switch, Try, TryPosition};
use super::segments::{ForkId, Graph, HEAD, SegmentId, WHOLE};

// `CodePathState` de ESLint para un camino (el programa, una función, un bloque estático o el
// inicializador de un campo de clase).
#[derive(Debug)]
pub struct State {
    pub fork: ForkId,
    pub choices: Vec<Choice>,
    pub chains: Vec<usize>,
    pub breaks: Vec<Break>,
    pub loops: Vec<Loop>,
    pub switches: Vec<Switch>,
    pub tries: Vec<Try>,
    pub current: Vec<SegmentId>,
    pub returned: Vec<SegmentId>,
    pub thrown: Vec<SegmentId>,
    pub finals: Vec<SegmentId>,
}

#[derive(Clone, Copy)]
enum Exit {
    Try(usize),
    Path,
}

impl State {
    pub fn new(graph: &mut Graph) -> Self {
        Self {
            fork: graph.new_root_fork(),
            choices: Vec::new(),
            chains: Vec::new(),
            breaks: Vec::new(),
            loops: Vec::new(),
            switches: Vec::new(),
            tries: Vec::new(),
            current: Vec::new(),
            returned: Vec::new(),
            thrown: Vec::new(),
            finals: Vec::new(),
        }
    }

    pub fn push_fork(&mut self, graph: &mut Graph, forks_leaving_path: bool) -> ForkId {
        self.fork = graph.new_empty_fork(self.fork, forks_leaving_path);
        self.fork
    }

    pub fn pop_fork(&mut self, graph: &mut Graph) {
        let last = self.fork;
        self.fork = graph.upper(last).unwrap_or(last);
        let next = graph.make_next(last, WHOLE);
        graph.replace_head(self.fork, next);
    }

    pub fn fork_path(&self, graph: &mut Graph) {
        if let Some(parent) = graph.upper(self.fork) {
            let next = graph.make_next(parent, HEAD);
            graph.add(self.fork, next);
        }
    }

    pub fn fork_bypass_path(&self, graph: &mut Graph) {
        if let Some(parent) = graph.upper(self.fork) {
            let head = graph.head(parent);
            graph.add(self.fork, head);
        }
    }

    pub fn replace_head_with_next(&self, graph: &mut Graph, fork: ForkId, range: (isize, isize)) {
        let next = graph.make_next(fork, range);
        graph.replace_head(self.fork, next);
    }

    pub fn push_choice(&mut self, graph: &mut Graph, kind: ChoiceKind, forks_as_result: bool) {
        self.choices.push(Choice {
            kind,
            forks_as_result,
            true_fork: graph.new_empty_fork(self.fork, false),
            false_fork: graph.new_empty_fork(self.fork, false),
            nullish_fork: graph.new_empty_fork(self.fork, false),
            processed: false,
        });
    }

    pub fn pop_choice(&mut self, graph: &mut Graph) -> Option<Choice> {
        let choice = self.choices.pop()?;
        let head = graph.head(self.fork);
        match choice.kind {
            ChoiceKind::And | ChoiceKind::Or | ChoiceKind::Nullish => {
                if !choice.processed {
                    add_to_branches(graph, &choice, &head);
                }
                if choice.forks_as_result {
                    self.merge_into_parent_choice(graph, &choice);
                    return Some(choice);
                }
            }
            ChoiceKind::Test => {
                let target = if choice.processed {
                    choice.false_fork
                } else {
                    choice.true_fork
                };
                graph.clear(target);
                graph.add(target, head);
            }
            ChoiceKind::Loop => return Some(choice),
        }
        graph.add_all(choice.true_fork, choice.false_fork);
        self.replace_head_with_next(graph, choice.true_fork, WHOLE);
        Some(choice)
    }

    fn merge_into_parent_choice(&mut self, graph: &mut Graph, choice: &Choice) {
        let Some(parent) = self.choices.last_mut() else {
            return;
        };
        graph.add_all(parent.true_fork, choice.true_fork);
        graph.add_all(parent.false_fork, choice.false_fork);
        graph.add_all(parent.nullish_fork, choice.nullish_fork);
        parent.processed = true;
    }

    pub fn make_logical_right(&mut self, graph: &mut Graph) {
        let fork = self.fork;
        let Some(choice) = self.choices.last_mut() else {
            return;
        };
        if choice.processed {
            let previous = match choice.kind {
                ChoiceKind::And => choice.true_fork,
                ChoiceKind::Or => choice.false_fork,
                ChoiceKind::Nullish | ChoiceKind::Test | ChoiceKind::Loop => choice.nullish_fork,
            };
            choice.processed = false;
            let next = graph.make_next(previous, WHOLE);
            graph.replace_head(fork, next);
            graph.clear(previous);
            return;
        }
        let head = graph.head(fork);
        let short_circuits: &[ForkId] = match choice.kind {
            ChoiceKind::And => &[choice.false_fork, choice.nullish_fork],
            ChoiceKind::Or => &[choice.true_fork],
            ChoiceKind::Nullish | ChoiceKind::Test | ChoiceKind::Loop => {
                &[choice.true_fork, choice.false_fork]
            }
        };
        for target in short_circuits {
            graph.add(*target, head.clone());
        }
        let next = graph.make_next(fork, HEAD);
        graph.replace_head(fork, next);
    }

    pub fn make_if_consequent(&mut self, graph: &mut Graph) {
        let fork = self.fork;
        let Some(choice) = self.choices.last_mut() else {
            return;
        };
        if !choice.processed {
            let head = graph.head(fork);
            add_to_branches(graph, choice, &head);
        }
        choice.processed = false;
        let next = graph.make_next(choice.true_fork, WHOLE);
        graph.replace_head(fork, next);
    }

    pub fn make_if_alternate(&mut self, graph: &mut Graph) {
        let fork = self.fork;
        let Some(choice) = self.choices.last_mut() else {
            return;
        };
        let head = graph.head(fork);
        graph.clear(choice.true_fork);
        graph.add(choice.true_fork, head);
        choice.processed = true;
        let next = graph.make_next(choice.false_fork, WHOLE);
        graph.replace_head(fork, next);
    }

    pub fn push_chain(&mut self) {
        self.chains.push(0);
    }

    pub fn pop_chain(&mut self, graph: &mut Graph) {
        let count = self.chains.pop().unwrap_or_default();
        for _ in 0..count {
            self.pop_choice(graph);
        }
    }

    pub fn make_optional_node(&mut self, graph: &mut Graph) {
        if let Some(count) = self.chains.last_mut() {
            *count = count.saturating_add(1);
            self.push_choice(graph, ChoiceKind::Nullish, false);
        }
    }

    pub fn make_optional_right(&mut self, graph: &mut Graph) {
        if !self.chains.is_empty() {
            self.make_logical_right(graph);
        }
    }

    pub fn push_break(&mut self, graph: &mut Graph, breakable: bool, label: Option<String>) -> ForkId {
        let broken = graph.new_empty_fork(self.fork, false);
        self.breaks.push(Break {
            breakable,
            label,
            broken,
        });
        broken
    }

    pub fn pop_break(&mut self, graph: &mut Graph) -> Option<Break> {
        let context = self.breaks.pop()?;
        if !context.breakable && !graph.is_empty(context.broken) {
            let head = graph.head(self.fork);
            graph.add(context.broken, head);
            self.replace_head_with_next(graph, context.broken, WHOLE);
        }
        Some(context)
    }

    pub fn make_head_unreachable(&self, graph: &mut Graph) {
        let unreachable = graph.make_unreachable(self.fork, HEAD);
        graph.replace_head(self.fork, unreachable);
    }

    pub fn make_break(&self, graph: &mut Graph, label: Option<&str>) {
        if !graph.is_fork_reachable(self.fork) {
            return;
        }
        let target =
            self.breaks.iter().rev().find(|context| {
                label.map_or(context.breakable, |label| context.label.as_deref() == Some(label))
            });
        if let Some(target) = target {
            graph.add(target.broken, graph.head(self.fork));
        }
        self.make_head_unreachable(graph);
    }

    pub fn make_continue(&self, graph: &mut Graph, label: Option<&str>) {
        if !graph.is_fork_reachable(self.fork) {
            return;
        }
        let target = label.map_or_else(
            || self.loops.last(),
            |label| {
                self.loops
                    .iter()
                    .rev()
                    .find(|context| context.label.as_deref() == Some(label))
            },
        );
        if let Some(target) = target {
            let head = graph.head(self.fork);
            continue_to(graph, target, head);
        }
        self.make_head_unreachable(graph);
    }

    fn return_target(&self) -> Exit {
        self.tries
            .iter()
            .rposition(|context| context.has_finalizer && context.position != TryPosition::Finally)
            .map_or(Exit::Path, Exit::Try)
    }

    fn throw_target(&self) -> Exit {
        self.tries
            .iter()
            .rposition(|context| {
                context.position == TryPosition::Try
                    || (context.has_finalizer && context.position == TryPosition::Catch)
            })
            .map_or(Exit::Path, Exit::Try)
    }

    pub fn add_returned(&mut self, graph: &mut Graph, segments: Vec<SegmentId>) {
        if let Exit::Try(index) = self.return_target() {
            if let Some(returned) = self.tries.get(index).and_then(|context| context.returned) {
                graph.add(returned, segments);
            }
            return;
        }
        add_leaving(&mut self.returned, &self.thrown, &mut self.finals, &segments);
    }

    pub fn add_thrown(&mut self, graph: &mut Graph, segments: Vec<SegmentId>) {
        if let Exit::Try(index) = self.throw_target() {
            if let Some(context) = self.tries.get(index) {
                graph.add(context.thrown, segments);
            }
            return;
        }
        add_leaving(&mut self.thrown, &self.returned, &mut self.finals, &segments);
    }

    pub fn make_return(&mut self, graph: &mut Graph) {
        if graph.is_fork_reachable(self.fork) {
            let head = graph.head(self.fork);
            self.add_returned(graph, head);
            self.make_head_unreachable(graph);
        }
    }

    pub fn make_throw(&mut self, graph: &mut Graph) {
        if graph.is_fork_reachable(self.fork) {
            let head = graph.head(self.fork);
            self.add_thrown(graph, head);
            self.make_head_unreachable(graph);
        }
    }

    pub fn make_yield(&mut self, graph: &mut Graph) {
        if graph.is_fork_reachable(self.fork) {
            let head = graph.head(self.fork);
            self.add_returned(graph, head.clone());
            self.add_thrown(graph, head);
            self.replace_head_with_next(graph, self.fork, HEAD);
        }
    }

    // `makeFirstThrowablePathInTryOrCatchBlock`: lo primero que puede tirar en un `try` abre el
    // camino hacia el `catch` (o el `finally`).
    pub fn make_first_throwable(&self, graph: &mut Graph) {
        if !graph.is_fork_reachable(self.fork) {
            return;
        }
        let Exit::Try(index) = self.throw_target() else {
            return;
        };
        let Some(thrown) = self
            .tries
            .get(index)
            .map(|context| context.thrown)
            .filter(|thrown| graph.is_empty(*thrown))
        else {
            return;
        };
        graph.add(thrown, graph.head(self.fork));
        self.replace_head_with_next(graph, self.fork, HEAD);
    }

    pub fn make_final(&mut self, graph: &Graph) {
        if self
            .current
            .first()
            .is_some_and(|segment| graph.is_reachable(*segment))
        {
            let current = self.current.clone();
            add_leaving(&mut self.returned, &self.thrown, &mut self.finals, &current);
        }
    }
}

fn add_to_branches(graph: &mut Graph, choice: &Choice, head: &[SegmentId]) {
    for fork in [choice.true_fork, choice.false_fork, choice.nullish_fork] {
        graph.add(fork, head.to_vec());
    }
}

fn continue_to(graph: &mut Graph, target: &Loop, head: Vec<SegmentId>) {
    match &target.continue_destination {
        Some(destination) => {
            graph.make_looped(&head, destination);
            if target.kind == LoopKind::ForIn {
                graph.add(target.broken, head);
            }
        }
        None => {
            if let Some(continue_fork) = target.continue_fork {
                graph.add(continue_fork, head);
            }
        }
    }
}

fn add_leaving(
    destination: &mut Vec<SegmentId>,
    others: &[SegmentId],
    finals: &mut Vec<SegmentId>,
    segments: &[SegmentId],
) {
    for segment in segments {
        destination.push(*segment);
        if !others.contains(segment) {
            finals.push(*segment);
        }
    }
}
