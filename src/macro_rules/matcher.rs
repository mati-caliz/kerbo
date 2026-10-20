use std::collections::HashMap;

use super::definition::{FragmentKind, Matcher, Repeat, Repetition, Rule};
use super::fragments::fragment_length;
use super::tokens::{Leaf, TokenTree};

#[derive(Debug, Clone)]
pub enum Binding {
    Fragment(Vec<TokenTree>, FragmentKind),
    Repeated(Vec<Self>),
}

pub type Bindings = HashMap<String, Binding>;

type Continuation<'continuation> = dyn FnMut(usize, &Bindings) -> Option<Bindings> + 'continuation;

#[derive(Debug, Clone, Copy)]
struct At<'input> {
    input: &'input [TokenTree],
    position: usize,
}

impl At<'_> {
    const fn moved(self, position: usize) -> Self {
        Self { position, ..self }
    }
}

pub fn match_rule(rule: &Rule, input: &[TokenTree]) -> Option<Bindings> {
    match_items(
        &rule.matcher,
        At { input, position: 0 },
        &Bindings::new(),
        &mut |end, found| (end == input.len()).then(|| found.clone()),
    )
}

fn match_items(
    items: &[Matcher],
    at: At<'_>,
    bindings: &Bindings,
    then: &mut Continuation<'_>,
) -> Option<Bindings> {
    let At { input, position } = at;
    let Some((item, rest)) = items.split_first() else {
        return then(position, bindings);
    };
    let remaining = input.get(position..)?;
    let next = position.saturating_add(1);
    match item {
        Matcher::Token(expected) => {
            let found = remaining.first()?.leaf()?;
            same_token(found, expected).then_some(())?;
            match_items(rest, at.moved(next), bindings, then)
        }
        Matcher::Group(delimiter, inner) => {
            let group = remaining
                .first()?
                .group()
                .filter(|group| group.delimiter == *delimiter)?;
            let length = group.trees.len();
            let inside_at = At {
                input: &group.trees,
                position: 0,
            };
            let inside = match_items(inner, inside_at, bindings, &mut |end, found| {
                (end == length).then(|| found.clone())
            })?;
            match_items(rest, at.moved(next), &inside, then)
        }
        Matcher::Fragment(name, kind) => {
            let length = fragment_length(*kind, remaining)?;
            let mut extended = bindings.clone();
            extended.insert(
                name.clone(),
                Binding::Fragment(remaining.get(..length)?.to_vec(), *kind),
            );
            match_items(rest, at.moved(position.saturating_add(length)), &extended, then)
        }
        Matcher::Repetition(repetition) => {
            let iterations = iterations(repetition, at);
            let minimum = usize::from(repetition.repeat == Repeat::OneOrMore);
            (minimum..=iterations.len()).rev().find_map(|count| {
                let taken = iterations.get(..count)?;
                let end = taken.last().map_or(position, |(end, _)| *end);
                let merged = merged(bindings, repetition, taken);
                match_items(rest, at.moved(end), &merged, then)
            })
        }
    }
}

// Cada vuelta toma la primera coincidencia y sólo se vuelve atrás en la cantidad de vueltas: así
// una repetición larga de `tt` no anida una llamada por token.
fn iterations(repetition: &Repetition<Matcher>, at: At<'_>) -> Vec<(usize, Bindings)> {
    let At { input, position } = at;
    let mut found: Vec<(usize, Bindings)> = Vec::new();
    let mut cursor = position;
    loop {
        if repetition.repeat == Repeat::ZeroOrOne && !found.is_empty() {
            break;
        }
        let start = match (&repetition.separator, found.is_empty()) {
            (Some(separator), false) => {
                if !input
                    .get(cursor)
                    .and_then(TokenTree::leaf)
                    .is_some_and(|leaf| same_token(leaf, separator))
                {
                    break;
                }
                cursor.saturating_add(1)
            }
            _ => cursor,
        };
        let mut end = None;
        let Some(bindings) = match_items(
            &repetition.items,
            at.moved(start),
            &Bindings::new(),
            &mut |reached, bindings| {
                end = Some(reached);
                Some(bindings.clone())
            },
        ) else {
            break;
        };
        let Some(end) = end.filter(|&end| end > cursor) else {
            break;
        };
        found.push((end, bindings));
        cursor = end;
    }
    found
}

fn merged(bindings: &Bindings, repetition: &Repetition<Matcher>, taken: &[(usize, Bindings)]) -> Bindings {
    let mut merged = bindings.clone();
    for name in variable_names(&repetition.items) {
        let values = taken
            .iter()
            .map(|(_, iteration)| {
                iteration
                    .get(&name)
                    .cloned()
                    .unwrap_or_else(|| Binding::Repeated(Vec::new()))
            })
            .collect();
        merged.insert(name, Binding::Repeated(values));
    }
    merged
}

fn variable_names(items: &[Matcher]) -> Vec<String> {
    items
        .iter()
        .flat_map(|item| match item {
            Matcher::Fragment(name, _) => vec![name.clone()],
            Matcher::Group(_, inner) => variable_names(inner),
            Matcher::Repetition(repetition) => variable_names(&repetition.items),
            Matcher::Token(_) => Vec::new(),
        })
        .collect()
}

fn same_token(found: &Leaf, expected: &Leaf) -> bool {
    found.kind == expected.kind && found.text == expected.text
}
