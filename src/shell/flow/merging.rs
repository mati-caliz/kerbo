use std::collections::BTreeMap;
use std::rc::Rc;

use super::frames::Stack;
use super::state::{Functions, Reach, State, Variable, Variables};

pub fn merge_pair(stack: &Stack, left: State, right: &State) -> State {
    if left == *right {
        return left;
    }
    let blank = State::empty(Reach::Live);
    State {
        globals: merge_variables(left.globals, &right.globals, |name| {
            stack.lookup_global(&blank, name)
        }),
        locals: merge_variables(left.locals, &right.locals, |name| stack.lookup(&blank, name).0),
        prefix: merge_variables(left.prefix, &right.prefix, |name| stack.lookup(&blank, name).0),
        functions: merge_functions(stack, left.functions, &right.functions, &blank),
        reach: Reach::Live,
    }
}

fn merge_variables(left: Variables, right: &Variables, fallback: impl Fn(&str) -> Variable) -> Variables {
    if Rc::ptr_eq(&left, right) {
        return left;
    }
    let mut merged = BTreeMap::new();
    for (name, value) in left.iter() {
        let other = right.get(name).copied().unwrap_or_else(|| fallback(name));
        merged.insert(name.clone(), value.merge(other));
    }
    for (name, value) in right.iter() {
        if !left.contains_key(name) {
            merged.insert(name.clone(), value.merge(fallback(name)));
        }
    }
    Rc::new(merged)
}

fn merge_functions(stack: &Stack, left: Functions, right: &Functions, blank: &State) -> Functions {
    if Rc::ptr_eq(&left, right) {
        return left;
    }
    let mut merged = BTreeMap::new();
    for (name, targets) in left.iter() {
        let mut joined = targets.clone();
        joined.extend(
            right
                .get(name)
                .cloned()
                .unwrap_or_else(|| stack.function(blank, name)),
        );
        merged.insert(name.clone(), joined);
    }
    for (name, targets) in right.iter() {
        if !left.contains_key(name) {
            let mut joined = targets.clone();
            joined.extend(stack.function(blank, name));
            merged.insert(name.clone(), joined);
        }
    }
    Rc::new(merged)
}
