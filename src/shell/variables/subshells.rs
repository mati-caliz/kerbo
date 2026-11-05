use std::collections::{HashMap, HashSet};

use super::{Event, EventKind};
use crate::rules::ShellRule;
use crate::shell::context::{Findings, Position};

const IGNORED_REFERENCES: &[&str] = &["@", "*", "IFS"];

// `findSubshelled` de ShellCheck: SC2030 y SC2031. Lo asignado adentro de un subshell queda muerto al
// cerrarlo, hasta que otra asignación lo reviva; de lo asignado varias veces cuenta la primera.
pub fn check(events: &[Event], findings: &mut Findings) {
    let mut scopes: Vec<Vec<&Event>> = vec![Vec::new()];
    let mut dead: HashMap<&str, Position> = HashMap::new();
    let mut reported: HashSet<(ShellRule, usize)> = HashSet::new();
    for event in events {
        match event.kind {
            EventKind::Assignment if event.is_true_source => {
                if let Some(scope) = scopes.last_mut() {
                    scope.push(event);
                }
                dead.remove(event.name.as_str());
            }
            EventKind::Reference if !IGNORED_REFERENCES.contains(&event.name.as_str()) => {
                if let Some(write) = dead.get(event.name.as_str()) {
                    report_lost_change(*write, event, &mut reported, findings);
                }
            }
            EventKind::ScopeStart => scopes.push(Vec::new()),
            EventKind::ScopeEnd => {
                for assigned in scopes.pop().unwrap_or_default().into_iter().rev() {
                    dead.insert(assigned.name.as_str(), assigned.position);
                }
            }
            EventKind::Assignment | EventKind::Checked | EventKind::Reference => {}
        }
    }
}

fn report_lost_change(
    write: Position,
    read: &Event,
    reported: &mut HashSet<(ShellRule, usize)>,
    findings: &mut Findings,
) {
    for (rule, position) in [(ShellRule::Sc2030, write), (ShellRule::Sc2031, read.position)] {
        if reported.insert((rule, position.byte)) {
            findings.report_at(rule, position, &read.name);
        }
    }
}
