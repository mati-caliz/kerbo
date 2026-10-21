use super::yaml::{Entry, Value};
use super::{DEFINITION_KEYS, EXTENDS, HEALTHCHECK, is_disabled, violation};
use crate::deploy::Violation;
use crate::rules::ComposeRule;

const CPUS_KEY: &str = "cpus";
const DEPLOY_CPUS: [&str; 4] = ["deploy", "resources", "limits", "cpus"];
const RESTART_KEY: &str = "restart";
const DEPLOY_RESTART: [&str; 2] = ["deploy", "restart_policy"];
const IMAGE_KEY: &str = "image";
const DEPENDS_ON: &str = "depends_on";
const CONDITION: &str = "condition";
const SERVICE_HEALTHY: &str = "service_healthy";
const LATEST_TAG: &str = "latest";
const TAG_SEPARATOR: char = ':';
const DIGEST_SEPARATOR: char = '@';
const PATH_SEPARATOR: char = '/';
const VARIABLE_START: &str = "${";

pub fn violations(services: &[Entry]) -> Vec<Violation> {
    let checked: Vec<&str> = services
        .iter()
        .filter(|service| {
            service
                .value
                .get(HEALTHCHECK)
                .is_some_and(|check| !is_disabled(check))
        })
        .map(|service| service.key.as_str())
        .collect();
    let mut found = Vec::new();
    for service in services {
        found.extend(cpu_limit(service));
        found.extend(restart(service));
        found.extend(image_tag(service));
        found.extend(depends_on(service, &checked));
    }
    found
}

fn defines_a_container(service: &Entry) -> bool {
    let definition = &service.value;
    DEFINITION_KEYS.iter().any(|key| definition.get(key).is_some()) && definition.get(EXTENDS).is_none()
}

fn cpu_limit(service: &Entry) -> Option<Violation> {
    let limited = service.value.get(CPUS_KEY).is_some() || service.value.path(&DEPLOY_CPUS).is_some();
    (defines_a_container(service) && !limited)
        .then(|| violation(ComposeRule::CpuLimit, service.line, &service.key))
}

fn restart(service: &Entry) -> Option<Violation> {
    let configured =
        service.value.get(RESTART_KEY).is_some() || service.value.path(&DEPLOY_RESTART).is_some();
    (defines_a_container(service) && !configured)
        .then(|| violation(ComposeRule::Restart, service.line, &service.key))
}

fn image_tag(service: &Entry) -> Option<Violation> {
    let image = service.value.entry(IMAGE_KEY)?;
    let reference = image.value.scalar()?;
    unpinned(reference).then(|| violation(ComposeRule::ImageTag, image.line, reference))
}

fn unpinned(reference: &str) -> bool {
    if reference.contains(DIGEST_SEPARATOR) || reference.starts_with(VARIABLE_START) {
        return false;
    }
    let name = reference.rsplit(PATH_SEPARATOR).next().unwrap_or(reference);
    name.split_once(TAG_SEPARATOR)
        .is_none_or(|(_, tag)| tag == LATEST_TAG)
}

fn depends_on(service: &Entry, checked: &[&str]) -> Vec<Violation> {
    let Some(dependencies) = service.value.entry(DEPENDS_ON) else {
        return Vec::new();
    };
    let unhealthy_wait =
        |name: &str, condition: Option<&str>| checked.contains(&name) && condition != Some(SERVICE_HEALTHY);
    match &dependencies.value {
        Value::Sequence(items) => items
            .iter()
            .filter_map(Value::scalar)
            .filter(|name| unhealthy_wait(name, None))
            .map(|name| violation(ComposeRule::DependsOnHealthy, dependencies.line, name))
            .collect(),
        Value::Map(entries) => entries
            .iter()
            .filter(|entry| unhealthy_wait(&entry.key, entry.value.get(CONDITION).and_then(Value::scalar)))
            .map(|entry| violation(ComposeRule::DependsOnHealthy, entry.line, &entry.key))
            .collect(),
        Value::Scalar(_) => Vec::new(),
    }
}
