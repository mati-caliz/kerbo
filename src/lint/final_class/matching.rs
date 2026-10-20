use std::cmp::Reverse;

use super::Collected;

const SEPARATOR: u8 = b'.';
const SEPARATOR_TEXT: &str = ".";

pub fn nearest(collected: &Collected<'_>, name: &str, count: impl Fn(&str) -> usize) -> Option<usize> {
    let suffix = [SEPARATOR_TEXT, name].concat();
    let candidates = collected.current();
    let best = candidates
        .iter()
        .filter_map(|&index| Some((index, collected.classes.get(index)?)))
        .filter(|(_, class)| class.qualified.ends_with(&suffix))
        .min_by_key(|(_, class)| (Reverse(count(&class.qualified)), class.depth))
        .map(|(index, _)| index);
    best.or_else(|| collected.by_name.get(name).copied())
}

pub fn declaration_count(pattern: &str, candidate: &str) -> usize {
    pattern
        .bytes()
        .zip(candidate.bytes())
        .enumerate()
        .take_while(|(_, (left, right))| left == right)
        .filter(|(_, (left, _))| *left == SEPARATOR)
        .map(|(index, _)| index)
        .last()
        .unwrap_or_default()
}

pub fn anonymous_count(pattern: &str, candidate: &str) -> usize {
    let shortest = pattern.len().min(candidate.len());
    let counts_last = candidate.len() > shortest && candidate.as_bytes().get(shortest) == Some(&SEPARATOR);
    pattern
        .bytes()
        .zip(candidate.bytes())
        .enumerate()
        .take_while(|(_, (left, right))| left == right)
        .filter(|&(index, (left, _))| {
            (counts_last && index.saturating_add(1) == shortest) || left == SEPARATOR
        })
        .map(|(index, _)| index)
        .last()
        .unwrap_or_default()
}
