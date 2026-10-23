use super::segments::{Graph, HEAD, SegmentId, WHOLE};
use super::state::{ChoiceKind, Loop, LoopKind, State};

pub fn push_loop(state: &mut State, graph: &mut Graph, kind: LoopKind, label: Option<String>) {
    let fork = state.fork;
    let broken = state.push_break(graph, true, label.clone());
    if kind != LoopKind::ForIn {
        state.push_choice(graph, ChoiceKind::Loop, false);
    }
    let continue_fork = (kind == LoopKind::DoWhile).then(|| graph.new_empty_fork(fork, false));
    state.loops.push(Loop {
        kind,
        label,
        broken,
        test: None,
        continue_destination: None,
        continue_fork,
        entry: Vec::new(),
        end_of_init: Vec::new(),
        test_segments: None,
        end_of_test: None,
        update_segments: None,
        end_of_update: None,
        before_left: Vec::new(),
        left_segments: Vec::new(),
        end_of_left: Vec::new(),
    });
}

pub fn pop_loop(state: &mut State, graph: &mut Graph) {
    let Some(context) = state.loops.pop() else {
        return;
    };
    let Some(broken) = state.pop_break(graph).map(|popped| popped.broken) else {
        return;
    };
    let head = graph.head(state.fork);
    match context.kind {
        LoopKind::While | LoopKind::For => {
            state.pop_choice(graph);
            graph.make_looped(&head, context.continue_destination.as_deref().unwrap_or_default());
        }
        LoopKind::DoWhile => close_do_while(state, graph, &context, head),
        LoopKind::ForIn => {
            graph.add(broken, head.clone());
            graph.make_looped(&head, &context.left_segments);
        }
    }
    let next = if graph.is_empty(broken) {
        graph.make_unreachable(state.fork, HEAD)
    } else {
        graph.make_next(broken, WHOLE)
    };
    graph.replace_head(state.fork, next);
}

fn close_do_while(state: &mut State, graph: &mut Graph, context: &Loop, head: Vec<SegmentId>) {
    let Some(choice) = state.pop_choice(graph) else {
        return;
    };
    if !choice.processed {
        graph.add(choice.true_fork, head.clone());
        graph.add(choice.false_fork, head);
    }
    if context.test != Some(true) {
        graph.add_all(context.broken, choice.false_fork);
    }
    for segments in graph.segments_list(choice.true_fork) {
        graph.make_looped(&segments, &context.entry);
    }
}

pub fn make_while_test(state: &mut State, graph: &mut Graph, test: Option<bool>) {
    let segments = graph.make_next(state.fork, WHOLE);
    if let Some(context) = state.loops.last_mut() {
        context.test = test;
        context.continue_destination = Some(segments.clone());
    }
    graph.replace_head(state.fork, segments);
}

pub fn make_while_body(state: &State, graph: &mut Graph) {
    let head = graph.head(state.fork);
    let (Some(context), Some(choice)) = (state.loops.last(), state.choices.last()) else {
        return;
    };
    if !choice.processed {
        graph.add(choice.true_fork, head.clone());
        graph.add(choice.false_fork, head);
    }
    if context.test != Some(true) {
        graph.add_all(context.broken, choice.false_fork);
    }
    state.replace_head_with_next(graph, choice.true_fork, WHOLE);
}

pub fn make_do_while_body(state: &mut State, graph: &mut Graph) {
    let body = graph.make_next(state.fork, HEAD);
    if let Some(context) = state.loops.last_mut() {
        context.entry.clone_from(&body);
    }
    graph.replace_head(state.fork, body);
}

pub fn make_do_while_test(state: &mut State, graph: &mut Graph, test: Option<bool>) {
    let head = graph.head(state.fork);
    let Some(context) = state.loops.last_mut() else {
        return;
    };
    context.test = test;
    let Some(continue_fork) = context.continue_fork.filter(|fork| !graph.is_empty(*fork)) else {
        return;
    };
    graph.add(continue_fork, head);
    state.replace_head_with_next(graph, continue_fork, WHOLE);
}

pub fn make_for_test(state: &mut State, graph: &mut Graph, test: Option<bool>) {
    let end_of_init = graph.head(state.fork);
    let segments = graph.make_next(state.fork, HEAD);
    if let Some(context) = state.loops.last_mut() {
        context.test = test;
        context.end_of_init = end_of_init;
        context.test_segments = Some(segments.clone());
        context.continue_destination = Some(segments.clone());
    }
    graph.replace_head(state.fork, segments);
}

pub fn make_for_update(state: &mut State, graph: &mut Graph) {
    let head = graph.head(state.fork);
    if state
        .loops
        .last()
        .is_some_and(|context| context.test_segments.is_some())
    {
        finalize_for_test(state, graph, &head);
    } else if let Some(context) = state.loops.last_mut() {
        context.end_of_init = head;
    }
    let segments = graph.make_disconnected(state.fork, HEAD);
    if let Some(context) = state.loops.last_mut() {
        context.update_segments = Some(segments.clone());
        context.continue_destination = Some(segments.clone());
    }
    graph.replace_head(state.fork, segments);
}

pub fn make_for_body(state: &mut State, graph: &mut Graph) {
    let head = graph.head(state.fork);
    let (has_update, has_test) = state.loops.last().map_or((false, false), |context| {
        (context.update_segments.is_some(), context.test_segments.is_some())
    });
    if has_update {
        close_for_update(state, graph, head);
    } else if has_test {
        finalize_for_test(state, graph, &head);
    } else if let Some(context) = state.loops.last_mut() {
        context.end_of_init = head;
    }
    let fork = state.fork;
    let Some(context) = state.loops.last_mut() else {
        return;
    };
    let body = if let Some(end_of_test) = &context.end_of_test {
        end_of_test.clone()
    } else {
        let previous = graph.new_empty_fork(fork, false);
        graph.add(previous, context.end_of_init.clone());
        if let Some(end_of_update) = &context.end_of_update {
            graph.add(previous, end_of_update.clone());
        }
        graph.make_next(previous, WHOLE)
    };
    if context.continue_destination.is_none() {
        context.continue_destination = Some(body.clone());
    }
    graph.replace_head(fork, body);
}

fn close_for_update(state: &mut State, graph: &mut Graph, head: Vec<SegmentId>) {
    let Some(context) = state.loops.last_mut() else {
        return;
    };
    if let Some(test_segments) = &context.test_segments {
        graph.make_looped(&head, test_segments);
    }
    context.end_of_update = Some(head);
}

// `finalizeTestSegmentsOfFor`: lo que sigue a la condición del `for` va al cuerpo o sale.
fn finalize_for_test(state: &mut State, graph: &mut Graph, head: &[SegmentId]) {
    let Some(choice) = state.choices.last() else {
        return;
    };
    let (true_fork, false_fork) = (choice.true_fork, choice.false_fork);
    if !choice.processed {
        graph.add(true_fork, head.to_vec());
        graph.add(false_fork, head.to_vec());
    }
    let end_of_test = graph.make_next(true_fork, WHOLE);
    let Some(context) = state.loops.last_mut() else {
        return;
    };
    if context.test != Some(true) {
        graph.add_all(context.broken, false_fork);
    }
    context.end_of_test = Some(end_of_test);
}

pub fn make_for_in_left(state: &mut State, graph: &mut Graph) {
    let before = graph.head(state.fork);
    let left = graph.make_disconnected(state.fork, HEAD);
    if let Some(context) = state.loops.last_mut() {
        context.before_left = before;
        context.left_segments.clone_from(&left);
        context.continue_destination = Some(left.clone());
    }
    graph.replace_head(state.fork, left);
}

pub fn make_for_in_right(state: &mut State, graph: &mut Graph) {
    let head = graph.head(state.fork);
    let fork = state.fork;
    let Some(context) = state.loops.last_mut() else {
        return;
    };
    let temporary = graph.new_empty_fork(fork, false);
    graph.add(temporary, context.before_left.clone());
    let right = graph.make_next(temporary, HEAD);
    context.end_of_left = head;
    graph.replace_head(fork, right);
}

pub fn make_for_in_body(state: &State, graph: &mut Graph) {
    let head = graph.head(state.fork);
    let Some(context) = state.loops.last() else {
        return;
    };
    let temporary = graph.new_empty_fork(state.fork, false);
    graph.add(temporary, context.end_of_left.clone());
    let body = graph.make_next(temporary, HEAD);
    graph.make_looped(&head, &context.left_segments);
    graph.add(context.broken, head);
    graph.replace_head(state.fork, body);
}
