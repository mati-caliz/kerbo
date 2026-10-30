mod commands;
mod expansions;
mod features;
mod redirections;
mod test_operators;

use std::collections::HashSet;

use tree_sitter::Node;

use crate::rules::ShellRule;
use crate::shell::context::{Context, Findings, Position};
use crate::shell::dialect::Dialect;
use crate::shell::variables::{EventKind, flow};

// Los `checkBashisms` de ShellCheck 0.11.0: lo que bash agrega y un script de sh, dash o busybox no
// tiene.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Posix {
    Sh,
    Dash,
    Busybox,
}

impl Posix {
    const fn of(dialect: Dialect) -> Option<Self> {
        match dialect {
            Dialect::Sh => Some(Self::Sh),
            Dialect::Dash => Some(Self::Dash),
            Dialect::Busybox => Some(Self::Busybox),
            Dialect::Bash | Dialect::Ksh => None,
        }
    }

    // ShellCheck trata a busybox como dash salvo en lo que busybox sí agrega.
    pub const fn is_dash(self) -> bool {
        !matches!(self, Self::Sh)
    }

    pub const fn is_busybox(self) -> bool {
        matches!(self, Self::Busybox)
    }
}

#[derive(Debug)]
pub struct Bashisms {
    shell: Posix,
    assigned: HashSet<String>,
}

impl Bashisms {
    pub fn new(context: &Context<'_, '_>) -> Option<Self> {
        let shell = Posix::of(context.dialect)?;
        let assigned = flow(context.root, context.source, context.associative.clone())
            .events
            .into_iter()
            .filter(|event| event.kind == EventKind::Assignment)
            .map(|event| event.name)
            .collect();
        Some(Self { shell, assigned })
    }

    pub fn check(&self, node: Node<'_>, context: &Context<'_, '_>, findings: &mut Findings) {
        let mut found = Found {
            shell: self.shell,
            findings,
        };
        features::check(node, context, &mut found);
        redirections::check(node, context, &mut found);
        expansions::check(node, context, &self.assigned, &mut found);
        test_operators::check(node, context, &mut found);
        commands::check(node, context, &mut found);
    }
}

// Los avisos de una pasada, con el dialecto a mano para las excepciones de dash y busybox.
pub struct Found<'findings> {
    shell: Posix,
    findings: &'findings mut Findings,
}

impl Found<'_> {
    pub const fn shell(&self) -> Posix {
        self.shell
    }

    pub fn report(&mut self, rule: ShellRule, node: Node<'_>, subject: &str) {
        self.findings.report(rule, node, subject);
    }

    pub fn report_at(&mut self, rule: ShellRule, position: Position, subject: &str) {
        self.findings.report_at(rule, position, subject);
    }
}
