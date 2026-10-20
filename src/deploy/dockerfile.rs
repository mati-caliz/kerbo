mod image;
mod instructions;
mod packages;
mod parser;
pub mod severity;
mod shell;
mod stages;

use parser::{Instruction, Keyword, parse};
use shell::Script;

use super::Violation;
use crate::rules::{DockerfileRule, RuleId};

const FIRST_COLUMN: usize = 1;
const MOUNT_FLAG: &str = "mount";
const MOUNT_OPTION_SEPARATOR: char = ',';
const MOUNT_VALUE_SEPARATOR: char = '=';
const MOUNT_TYPE: &str = "type";
const CACHING_MOUNTS: &[&str] = &["cache", "tmpfs"];
const MOUNT_TARGETS: &[&str] = &["target", "dst", "destination"];
const QUOTES: &[char] = &['"', '\''];

#[derive(Debug)]
struct Step {
    instruction: Instruction,
    script: Option<Script>,
}

#[derive(Debug, Default)]
struct Findings {
    violations: Vec<Violation>,
}

impl Findings {
    fn report(&mut self, rule: DockerfileRule, line: usize, subject: impl Into<String>) {
        self.violations.push(Violation {
            rule: RuleId::Dockerfile(rule),
            line,
            column: FIRST_COLUMN,
            subject: subject.into(),
        });
    }
}

pub fn violations(source: &str) -> Vec<Violation> {
    let steps: Vec<Step> = parse(source)
        .into_iter()
        .map(|instruction| {
            let script = (instruction.keyword == Keyword::Run).then(|| shell::parse(&instruction.script()));
            Step { instruction, script }
        })
        .collect();
    let mut findings = Findings::default();
    instructions::check(&steps, &mut findings);
    packages::check(&steps, &mut findings);
    stages::check(&steps, &mut findings);
    let mut violations = findings.violations;
    violations.sort_by_key(|violation| violation.line);
    violations
}

fn has_caching_mount(instruction: &Instruction, fragment: &str) -> bool {
    instruction
        .flags
        .iter()
        .filter(|flag| flag.name == MOUNT_FLAG)
        .any(|flag| {
            let options: Vec<(&str, &str)> = flag
                .value
                .split(MOUNT_OPTION_SEPARATOR)
                .filter_map(|option| option.split_once(MOUNT_VALUE_SEPARATOR))
                .collect();
            let caching = options
                .iter()
                .any(|(key, value)| *key == MOUNT_TYPE && CACHING_MOUNTS.contains(value));
            let targeted = options
                .iter()
                .any(|(key, value)| MOUNT_TARGETS.contains(key) && value.contains(fragment));
            caching && targeted
        })
}

fn unquoted(text: &str) -> &str {
    text.trim_matches(QUOTES)
}

#[cfg(test)]
mod tests;
