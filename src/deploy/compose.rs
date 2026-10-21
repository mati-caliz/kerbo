mod duration;
mod services;
mod yaml;

use std::time::Duration;

use tree_sitter::Node;

use super::Violation;
use crate::rules::{ComposeRule, RuleId};
use yaml::{Entry, Value};

const FIRST_COLUMN: usize = 1;
const SERVICES: &str = "services";
const MEMORY_KEY: &str = "mem_limit";
const DEPLOY_MEMORY: [&str; 4] = ["deploy", "resources", "limits", "memory"];
const DEFINITION_KEYS: &[&str] = &["image", "build"];
const EXTENDS: &str = "extends";
const HEALTHCHECK: &str = "healthcheck";
const DISABLE: &str = "disable";
const TRUE: &str = "true";
const TEST: &str = "test";
const NONE: &str = "NONE";
const START_PERIOD: &str = "start_period";
const INTERVAL: &str = "interval";
const TIMEOUT: &str = "timeout";
const MIN_INTERVAL: Duration = Duration::from_secs(5);
const MAX_INTERVAL: Duration = Duration::from_secs(300);
const DEFAULT_INTERVAL: Duration = Duration::from_secs(30);

pub fn violations(root: Node<'_>, source: &[u8]) -> Vec<Violation> {
    let Some(document) = yaml::document(root, source) else {
        return Vec::new();
    };
    let services = document.get(SERVICES).map(Value::entries).unwrap_or_default();
    let mut found = services::violations(services);
    for service in services {
        found.extend(memory_limit(service));
        found.extend(healthcheck(service));
    }
    found.sort_by_key(|violation| violation.line);
    found
}

fn violation(rule: ComposeRule, line: usize, subject: &str) -> Violation {
    Violation {
        rule: RuleId::Compose(rule),
        line,
        column: FIRST_COLUMN,
        subject: subject.to_owned(),
        fix: None,
    }
}

fn memory_limit(service: &Entry) -> Option<Violation> {
    let definition = &service.value;
    let complete = DEFINITION_KEYS.iter().any(|key| definition.get(key).is_some());
    let limited = definition.get(MEMORY_KEY).is_some() || definition.path(&DEPLOY_MEMORY).is_some();
    let inherits = definition.get(EXTENDS).is_some();
    (complete && !limited && !inherits)
        .then(|| violation(ComposeRule::MemoryLimit, service.line, &service.key))
}

fn healthcheck(service: &Entry) -> Vec<Violation> {
    let Some(check) = service.value.entry(HEALTHCHECK) else {
        return Vec::new();
    };
    if is_disabled(&check.value) {
        return Vec::new();
    }
    let mut found = Vec::new();
    if check.value.get(START_PERIOD).is_none() {
        found.push(violation(
            ComposeRule::HealthcheckStartPeriod,
            check.line,
            &service.key,
        ));
    }
    let interval_entry = check.value.entry(INTERVAL);
    let interval = interval_entry.map_or(Some(DEFAULT_INTERVAL), |entry| {
        entry.value.scalar().and_then(duration::parse)
    });
    let timeout = check
        .value
        .get(TIMEOUT)
        .and_then(Value::scalar)
        .and_then(duration::parse);
    let unreasonable = interval.is_some_and(|interval| {
        interval < MIN_INTERVAL
            || interval > MAX_INTERVAL
            || timeout.is_some_and(|timeout| timeout >= interval)
    });
    if unreasonable {
        let line = interval_entry.map_or(check.line, |entry| entry.line);
        found.push(violation(ComposeRule::HealthcheckInterval, line, &service.key));
    }
    found
}

fn is_disabled(check: &Value) -> bool {
    let disabled = check.get(DISABLE).and_then(Value::scalar) == Some(TRUE);
    let test_is_none = match check.get(TEST) {
        Some(Value::Scalar(text)) => text == NONE,
        Some(Value::Sequence(items)) => items.first().and_then(Value::scalar) == Some(NONE),
        Some(Value::Map(_)) | None => false,
    };
    disabled || test_is_none
}

#[cfg(test)]
mod tests;
