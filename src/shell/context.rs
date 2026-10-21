use std::collections::HashSet;

use tree_sitter::{Node, Point};

use super::commands::Command;
use super::dialect::Dialect;
use super::syntax::{ASSIGNMENT, VARIABLE_NAME, descendants};
use super::variables::assigned_name;
use super::words::{literal, oversimplify};
use crate::fix::Fix;
use crate::rules::ShellRule;

const SET: &str = "set";
const ERREXIT_OPTION: &str = "errexit";
const ERREXIT_FLAG: &str = "e";
const FLAG_PREFIX: char = '-';
const WHITESPACE: &[char] = &[' ', '\t'];
const ERREXIT_LETTER: char = 'e';
const ASSOCIATIVE_DECLARERS: &[&str] = &["declare", "local", "typeset"];
const ASSOCIATIVE_FLAG: &str = "A";

#[derive(Debug, Clone)]
pub struct Context<'tree, 'source> {
    pub root: Node<'tree>,
    pub source: &'source [u8],
    pub dialect: Dialect,
    pub has_set_e: bool,
    pub shell_specified: bool,
    pub associative: HashSet<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct Origin<'shebang> {
    pub dialect: Dialect,
    pub shebang: Option<&'shebang str>,
    pub shell_specified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub rule: ShellRule,
    pub byte: usize,
    pub point: Point,
    pub subject: String,
    pub fix: Option<Fix>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub byte: usize,
    pub point: Point,
}

impl Position {
    pub fn of(node: Node<'_>) -> Self {
        Self {
            byte: node.start_byte(),
            point: node.start_position(),
        }
    }

    pub const fn shifted(self, bytes: usize) -> Self {
        Self {
            byte: self.byte.saturating_add(bytes),
            point: Point {
                row: self.point.row,
                column: self.point.column.saturating_add(bytes),
            },
        }
    }
}

#[derive(Debug, Default)]
pub struct Findings {
    pub found: Vec<Finding>,
}

impl Findings {
    pub fn report(&mut self, rule: ShellRule, node: Node<'_>, subject: &str) {
        self.report_at(rule, Position::of(node), subject);
    }

    pub fn report_fixed(&mut self, rule: ShellRule, node: Node<'_>, (subject, fix): (&str, Fix)) {
        self.found.push(Finding {
            rule,
            byte: node.start_byte(),
            point: node.start_position(),
            subject: subject.to_owned(),
            fix: Some(fix),
        });
    }

    pub fn report_at(&mut self, rule: ShellRule, position: Position, subject: &str) {
        self.found.push(Finding {
            rule,
            byte: position.byte,
            point: position.point,
            subject: subject.to_owned(),
            fix: None,
        });
    }
}

impl<'tree, 'source> Context<'tree, 'source> {
    pub fn new(root: Node<'tree>, source: &'source [u8], origin: &Origin<'_>) -> Self {
        let commands: Vec<Command<'tree>> = descendants(root).into_iter().filter_map(Command::of).collect();
        let mut context = Self {
            root,
            source,
            dialect: origin.dialect,
            has_set_e: origin.shebang.is_some_and(shebang_sets_errexit),
            shell_specified: origin.shell_specified,
            associative: commands
                .iter()
                .flat_map(|command| associative_names(command, source))
                .collect(),
        };
        context.has_set_e |= commands.iter().any(|command| context.sets_errexit(command));
        context
    }

    pub fn text(&self, node: Node<'_>) -> &'source str {
        super::syntax::text(node, self.source)
    }

    fn sets_errexit(&self, command: &Command<'_>) -> bool {
        command.is_exactly(SET, self.source)
            && (command
                .words
                .iter()
                .any(|word| oversimplify(*word, self.source) == ERREXIT_OPTION)
                || command.has_flag(ERREXIT_FLAG, self.source))
    }
}

fn associative_names(command: &Command<'_>, source: &[u8]) -> Vec<String> {
    let declares = command
        .literal_name(source)
        .is_some_and(|name| ASSOCIATIVE_DECLARERS.contains(&name.as_str()));
    if !declares || !command.has_flag(ASSOCIATIVE_FLAG, source) {
        return Vec::new();
    }
    command
        .flags(source)
        .into_iter()
        .filter(|(_, flag)| flag.is_empty())
        .filter_map(|(argument, _)| {
            if argument.kind() == ASSIGNMENT {
                assigned_name(argument, source)
            } else if argument.kind() == VARIABLE_NAME {
                Some(super::syntax::text(argument, source).to_owned())
            } else {
                literal(argument, source)
            }
        })
        .collect()
}

fn shebang_sets_errexit(shebang: &str) -> bool {
    shebang.match_indices(WHITESPACE).any(|(index, _)| {
        shebang
            .get(index.saturating_add(1)..)
            .and_then(|rest| rest.strip_prefix(FLAG_PREFIX))
            .is_some_and(|rest| {
                rest.chars()
                    .take_while(|letter| *letter != FLAG_PREFIX)
                    .any(|letter| letter == ERREXIT_LETTER)
            })
    })
}
