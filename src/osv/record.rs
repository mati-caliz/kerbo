use std::cmp::Ordering;

use serde::Deserialize;

use super::lockfiles::Dependency;
use super::version::compare;
use super::{Ecosystem, normalized_name};

const INTRODUCED_FROM_START: &str = "0";
const GIT_RANGE: &str = "GIT";
const SUMMARY_LIMIT: usize = 160;
const ELLIPSIS: char = '…';

#[derive(Debug, Clone, Deserialize)]
pub struct Record {
    pub id: String,
    #[serde(default)]
    pub modified: String,
    pub withdrawn: Option<String>,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub details: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub affected: Vec<Affected>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Affected {
    package: Option<PackageName>,
    #[serde(default)]
    ranges: Vec<Range>,
    #[serde(default)]
    versions: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct PackageName {
    name: String,
    ecosystem: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Range {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    events: Vec<Event>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct Event {
    introduced: Option<String>,
    fixed: Option<String>,
    last_affected: Option<String>,
}

impl Record {
    pub fn summary(&self) -> String {
        let text = if self.summary.trim().is_empty() {
            self.details
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .unwrap_or_default()
        } else {
            self.summary.trim()
        };
        if text.chars().count() <= SUMMARY_LIMIT {
            return text.to_owned();
        }
        let mut shortened: String = text.chars().take(SUMMARY_LIMIT).collect();
        shortened.push(ELLIPSIS);
        shortened
    }

    pub fn affects(&self, dependency: &Dependency) -> bool {
        self.withdrawn.is_none()
            && self.entries_for(dependency).any(|affected| {
                affected.versions.contains(&dependency.version)
                    || affected
                        .ranges
                        .iter()
                        .any(|range| range_affects(range, dependency))
            })
    }

    pub fn fixed_version(&self, dependency: &Dependency) -> Option<String> {
        let ecosystem = dependency.ecosystem;
        self.entries_for(dependency)
            .flat_map(|affected| affected.ranges.iter())
            .filter(|range| range.kind != GIT_RANGE)
            .flat_map(|range| range.events.iter())
            .filter_map(|event| event.fixed.as_deref())
            .filter(|fixed| compare(ecosystem, fixed, &dependency.version) == Ordering::Greater)
            .min_by(|left, right| compare(ecosystem, left, right))
            .map(str::to_owned)
    }

    fn entries_for<'record>(
        &'record self,
        dependency: &'record Dependency,
    ) -> impl Iterator<Item = &'record Affected> + 'record {
        let wanted = normalized_name(dependency.ecosystem, &dependency.name);
        self.affected.iter().filter(move |affected| {
            affected.package.as_ref().is_some_and(|package| {
                package.ecosystem == dependency.ecosystem.as_str()
                    && normalized_name(dependency.ecosystem, &package.name) == wanted
            })
        })
    }
}

fn range_affects(range: &Range, dependency: &Dependency) -> bool {
    if range.kind == GIT_RANGE {
        return false;
    }
    let ecosystem = dependency.ecosystem;
    let mut events: Vec<&Event> = range.events.iter().collect();
    events.sort_by(|left, right| compare_events(ecosystem, left, right));
    let mut affected = false;
    for event in events {
        if let Some(introduced) = &event.introduced {
            affected = affected || at_least(ecosystem, &dependency.version, introduced);
        }
        if event
            .fixed
            .as_ref()
            .is_some_and(|fixed| at_least(ecosystem, &dependency.version, fixed))
        {
            affected = false;
        }
        if let Some(last) = &event.last_affected
            && compare(ecosystem, &dependency.version, last) == Ordering::Greater
        {
            affected = false;
        }
    }
    affected
}

fn at_least(ecosystem: Ecosystem, version: &str, bound: &str) -> bool {
    bound == INTRODUCED_FROM_START || compare(ecosystem, version, bound) != Ordering::Less
}

fn event_version(event: &Event) -> &str {
    event
        .introduced
        .as_deref()
        .or(event.fixed.as_deref())
        .or(event.last_affected.as_deref())
        .unwrap_or_default()
}

fn compare_events(ecosystem: Ecosystem, left: &Event, right: &Event) -> Ordering {
    let (left, right) = (event_version(left), event_version(right));
    match (left == INTRODUCED_FROM_START, right == INTRODUCED_FROM_START) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        (false, false) => compare(ecosystem, left, right),
    }
}

#[cfg(test)]
mod tests;
