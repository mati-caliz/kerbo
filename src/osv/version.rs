mod maven;
mod python;

use std::cmp::Ordering;

use super::Ecosystem;

const BUILD_SEPARATOR: char = '+';
const PRERELEASE_SEPARATOR: char = '-';
const PART_SEPARATOR: char = '.';
const PREFIXES: &[char] = &['v', 'V', '='];

pub fn compare(ecosystem: Ecosystem, left: &str, right: &str) -> Ordering {
    match ecosystem {
        Ecosystem::CratesIo | Ecosystem::Npm | Ecosystem::Go => compare_semver(left, right),
        Ecosystem::PyPi => python::compare(left, right),
        Ecosystem::Maven => maven::compare(left, right),
    }
}

pub fn compare_semver(left: &str, right: &str) -> Ordering {
    let (left_core, left_pre) = split_semver(left);
    let (right_core, right_pre) = split_semver(right);
    let numbers = |core: &str| -> Vec<u64> {
        core.split(PART_SEPARATOR)
            .map(|part| leading_number(part).unwrap_or_default())
            .collect()
    };
    compare_numbers(&numbers(left_core), &numbers(right_core)).then_with(|| match (left_pre, right_pre) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(left_identifiers), Some(right_identifiers)) => {
            compare_identifiers(left_identifiers, right_identifiers)
        }
    })
}

fn split_semver(version: &str) -> (&str, Option<&str>) {
    let trimmed = version.trim().trim_start_matches(PREFIXES);
    let without_build = trimmed.split(BUILD_SEPARATOR).next().unwrap_or(trimmed);
    match without_build.split_once(PRERELEASE_SEPARATOR) {
        Some((core, pre)) => (core, Some(pre)),
        None => (without_build, None),
    }
}

fn compare_identifiers(left: &str, right: &str) -> Ordering {
    let mut left_parts = left.split(PART_SEPARATOR);
    let mut right_parts = right.split(PART_SEPARATOR);
    loop {
        let order = match (left_parts.next(), right_parts.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => Ordering::Less,
            (Some(_), None) => Ordering::Greater,
            (Some(left_part), Some(right_part)) => compare_identifier(left_part, right_part),
        };
        if order != Ordering::Equal {
            return order;
        }
    }
}

fn compare_identifier(left: &str, right: &str) -> Ordering {
    match (whole_number(left), whole_number(right)) {
        (Some(left_number), Some(right_number)) => left_number.cmp(&right_number),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => left.cmp(right),
    }
}

pub fn compare_numbers(left: &[u64], right: &[u64]) -> Ordering {
    let length = left.len().max(right.len());
    (0..length)
        .map(|index| {
            let left = left.get(index).copied().unwrap_or_default();
            let right = right.get(index).copied().unwrap_or_default();
            left.cmp(&right)
        })
        .find(|order| *order != Ordering::Equal)
        .unwrap_or(Ordering::Equal)
}

pub fn whole_number(text: &str) -> Option<u64> {
    (!text.is_empty() && text.chars().all(|character| character.is_ascii_digit()))
        .then(|| text.parse().ok())
        .flatten()
}

pub fn leading_number(text: &str) -> Option<u64> {
    let digits: String = text.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

#[cfg(test)]
mod tests;
