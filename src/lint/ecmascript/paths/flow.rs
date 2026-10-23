use super::segments::{ForkId, Graph, HEAD, SegmentId, WHOLE};
use super::state::{State, Switch, Try, TryPosition};

const HALVES: usize = 2;

// Qué cuerpo de `case` se abre: si está vacío y si es el `default`.
#[derive(Debug, Clone, Copy)]
pub struct CaseBody {
    pub empty: bool,
    pub default: bool,
}

pub fn push_switch(state: &mut State, graph: &mut Graph, has_case: bool, label: Option<String>) {
    state.switches.push(Switch {
        has_case,
        ..Switch::default()
    });
    state.push_break(graph, true, label);
}

pub fn pop_switch(state: &mut State, graph: &mut Graph) {
    let Some(switch) = state.switches.pop() else {
        return;
    };
    let Some(broken) = state.pop_break(graph).map(|popped| popped.broken) else {
        return;
    };
    if switch.fork_count == 0 {
        close_switch_without_cases(state, graph, broken);
        return;
    }
    let last_segments = graph.head(state.fork);
    state.fork_bypass_path(graph);
    let last_case_segments = graph.head(state.fork);
    graph.add(broken, last_segments);
    if !switch.default_case.is_last {
        link_default(graph, &switch, broken, last_case_segments);
    }
    for _ in 0..switch.fork_count {
        state.fork = graph.upper(state.fork).unwrap_or(state.fork);
    }
    state.replace_head_with_next(graph, broken, WHOLE);
}

fn close_switch_without_cases(state: &State, graph: &mut Graph, broken: ForkId) {
    if graph.is_empty(broken) {
        return;
    }
    let next = graph.make_next(state.fork, HEAD);
    graph.add(broken, next);
    state.replace_head_with_next(graph, broken, WHOLE);
}

// Un `default` que no va al final sigue después del último caso.
fn link_default(graph: &mut Graph, switch: &Switch, broken: ForkId, last_case_segments: Vec<SegmentId>) {
    let Some(default_body) = &switch.default_body else {
        graph.add(broken, last_case_segments);
        return;
    };
    if let Some(default_segments) = &switch.default_segments {
        graph.disconnect(default_segments, default_body);
    }
    graph.make_looped(&last_case_segments, default_body);
}

pub fn make_switch_case_body(state: &mut State, graph: &mut Graph, body: CaseBody) {
    if !state.switches.last().is_some_and(|switch| switch.has_case) {
        return;
    }
    let parent = state.fork;
    let fork = state.push_fork(graph, false);
    let next = graph.make_next(parent, WHOLE);
    graph.add(fork, next);
    let parent_head = graph.head(parent);
    let body_head = graph.head(fork);
    let Some(switch) = state.switches.last_mut() else {
        return;
    };
    if body.default {
        switch.default_segments = Some(parent_head);
        if body.empty {
            switch.default_case.found_empty = true;
        } else {
            switch.default_body = Some(body_head);
        }
    } else if !body.empty && switch.default_case.found_empty {
        switch.default_case.found_empty = false;
        switch.default_body = Some(body_head);
    }
    switch.default_case.is_last = body.default;
    switch.fork_count = switch.fork_count.saturating_add(1);
}

pub fn push_try(state: &mut State, graph: &mut Graph, has_finalizer: bool) {
    let returned = has_finalizer.then(|| graph.new_empty_fork(state.fork, false));
    let thrown = graph.new_empty_fork(state.fork, false);
    state.tries.push(Try {
        has_finalizer,
        position: TryPosition::Try,
        returned,
        thrown,
    });
}

pub fn pop_try(state: &mut State, graph: &mut Graph) {
    let Some(context) = state.tries.pop() else {
        return;
    };
    if context.position == TryPosition::Catch {
        state.pop_fork(graph);
        return;
    }
    let returned_empty = context.returned.is_none_or(|returned| graph.is_empty(returned));
    let thrown_empty = graph.is_empty(context.thrown);
    if returned_empty && thrown_empty {
        return;
    }
    let head = graph.head(state.fork);
    state.fork = graph.upper(state.fork).unwrap_or(state.fork);
    let (normal, leaving) = head.split_at(head.len() / HALVES);
    if !returned_empty {
        state.add_returned(graph, leaving.to_vec());
    }
    if !thrown_empty {
        state.add_thrown(graph, leaving.to_vec());
    }
    graph.replace_head(state.fork, normal.to_vec());
}

pub fn make_catch_block(state: &mut State, graph: &mut Graph) {
    let fork = state.fork;
    let new_thrown = graph.new_empty_fork(fork, false);
    let Some(context) = state.tries.last_mut() else {
        return;
    };
    let original_thrown = context.thrown;
    context.position = TryPosition::Catch;
    context.thrown = new_thrown;
    let head = graph.head(fork);
    graph.add(original_thrown, head);
    let thrown_segments = graph.make_next(original_thrown, WHOLE);
    state.push_fork(graph, false);
    state.fork_bypass_path(graph);
    graph.add(state.fork, thrown_segments);
}

pub fn make_finally_block(state: &mut State, graph: &mut Graph) {
    let leaving_head = graph.head(state.fork);
    if state
        .tries
        .last()
        .is_some_and(|context| context.position == TryPosition::Catch)
    {
        state.pop_fork(graph);
    }
    let Some(context) = state.tries.last_mut() else {
        return;
    };
    context.position = TryPosition::Finally;
    let returned = context.returned;
    let thrown = context.thrown;
    let returned_empty = returned.is_none_or(|fork| graph.is_empty(fork));
    if returned_empty && graph.is_empty(thrown) {
        return;
    }
    let mut segments = graph.make_next(state.fork, HEAD);
    let returned_list = returned.map(|fork| graph.segments_list(fork)).unwrap_or_default();
    let thrown_list = graph.segments_list(thrown);
    for path in 0..graph.count(state.fork) {
        let previous: Vec<SegmentId> = leaving_head
            .get(path)
            .into_iter()
            .chain(returned_list.iter().filter_map(|listed| listed.get(path)))
            .chain(thrown_list.iter().filter_map(|listed| listed.get(path)))
            .copied()
            .collect();
        segments.push(graph.new_next(&previous));
    }
    state.push_fork(graph, true);
    graph.add(state.fork, segments);
}
