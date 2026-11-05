use std::collections::{BTreeMap, BTreeSet};

use tree_sitter::Node;

use super::{Event, EventKind};
use crate::rules::ShellRule;
use crate::shell::context::{Context, Findings, Position};
use crate::shell::environment::INTERNAL_VARIABLES;
use crate::shell::parameters::{is_variable_name, modifier, reference};
use crate::shell::syntax::{EXPANSIONS, ancestors};
use crate::shell::words::braced_content;

const ALTERNATIVE_MODIFIERS: &[&str] = &["+", ":+"];
const SHORT_MATCH_LENGTH: usize = 3;
const LONG_MATCH_LENGTH: usize = 7;
const SHORT_MATCH_SCORE: usize = 1;
const LONG_MATCH_SCORE: usize = 2;

// `checkUnassignedReferences` de ShellCheck: un nombre leído que nada asigna en todo el archivo, sólo
// con minúsculas (SC2154) o, si es todo mayúsculas, si se parece a uno asignado (SC2153).
pub fn check(events: &[Event], context: &Context<'_, '_>, findings: &mut Findings) {
    let mut first_reads: BTreeMap<&str, &Event> = BTreeMap::new();
    let mut written: BTreeSet<&str> = BTreeSet::new();
    for event in events {
        match event.kind {
            EventKind::Assignment | EventKind::Checked => {
                written.insert(event.name.as_str());
            }
            EventKind::Reference => {
                first_reads.entry(event.name.as_str()).or_insert(event);
            }
            EventKind::ScopeStart | EventKind::ScopeEnd => {}
        }
    }
    let written_names: Vec<&str> = written
        .iter()
        .copied()
        .filter(|name| is_variable_name(name))
        .collect();
    for (name, read) in first_reads {
        if written.contains(name)
            || INTERNAL_VARIABLES.contains(&name)
            || !is_variable_name(name)
            || read.is_guarded
            || is_inside_exempting_expansion(name, read.position, context)
        {
            continue;
        }
        if name.chars().any(char::is_lowercase) {
            findings.report_at(ShellRule::Sc2154, read.position, name);
        } else if best_match(name, &written_names).is_some() {
            findings.report_at(ShellRule::Sc2153, read.position, name);
        }
    }
}

// `isException`: el nombre es un índice de otra expansión (`${a[b]}`) o está protegido por una
// (`${b:+x$b}`).
fn is_inside_exempting_expansion(name: &str, position: Position, context: &Context<'_, '_>) -> bool {
    let Some(place) = context
        .root
        .descendant_for_byte_range(position.byte, position.byte)
    else {
        return false;
    };
    std::iter::once(place)
        .chain(ancestors(place))
        .filter(|node| EXPANSIONS.contains(&node.kind()))
        .any(|expansion| exempts(expansion, name, context.source))
}

fn exempts(expansion: Node<'_>, name: &str, source: &[u8]) -> bool {
    let braced = braced_content(expansion, source);
    let modified = modifier(&braced);
    reference(&braced) != name
        || ALTERNATIVE_MODIFIERS
            .iter()
            .any(|alternative| modified.starts_with(alternative))
}

fn best_match<'names>(name: &str, written: &[&'names str]) -> Option<&'names str> {
    let (candidate, score) = written
        .iter()
        .map(|candidate| (*candidate, match_score(name, candidate)))
        .min_by_key(|(_, score)| *score)?;
    let length = candidate.chars().count();
    ((length > SHORT_MATCH_LENGTH && score <= SHORT_MATCH_SCORE)
        || (length > LONG_MATCH_LENGTH && score <= LONG_MATCH_SCORE))
        .then_some(candidate)
}

fn match_score(name: &str, candidate: &str) -> usize {
    if name != candidate && name.eq_ignore_ascii_case(candidate) {
        1
    } else {
        edit_distance(name, candidate)
    }
}

fn edit_distance(left: &str, right: &str) -> usize {
    let right_characters: Vec<char> = right.chars().collect();
    let mut previous: Vec<usize> = (0..=right_characters.len()).collect();
    for (left_index, left_character) in left.chars().enumerate() {
        let mut current = vec![left_index.saturating_add(1)];
        for (right_index, right_character) in right_characters.iter().enumerate() {
            let diagonal = previous.get(right_index).copied().unwrap_or_default();
            let above = previous
                .get(right_index.saturating_add(1))
                .copied()
                .unwrap_or_default();
            let beside = current.last().copied().unwrap_or_default();
            let replaced = diagonal.saturating_add(usize::from(left_character != *right_character));
            current.push(
                replaced
                    .min(above.saturating_add(1))
                    .min(beside.saturating_add(1)),
            );
        }
        previous = current;
    }
    previous.last().copied().unwrap_or_default()
}
