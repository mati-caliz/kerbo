mod calls;
mod commands;
mod control;
mod frames;
mod merging;
mod state;
mod values;

use crate::shell::flow::calls::Calls;
use crate::shell::flow::control::Control;
use std::collections::{BTreeSet, HashMap, HashSet};

use tree_sitter::Node;

use super::commands::Command;
use super::context::Context;
use super::dialect::Dialect;
use super::environment::{INTERNAL_VARIABLES, SPECIAL_INTEGER_VARIABLES, WITHOUT_SPACES};
use super::parameters::reference;
use super::syntax::{FUNCTION, NAME_FIELD, descendants, text};
use super::words::{braced_content, oversimplify};
use frames::{Dependency, Frame, Stack};
use state::{Reach, Scope, State, Target, Variable};

const LASTPIPE: &str = "lastpipe";
const SET: &str = "set";
const SHOPT: &str = "shopt";
const SET_OPTION_FLAG: &str = "o";

pub fn clean_expansions(context: &Context<'_, '_>, candidates: &[Node<'_>]) -> HashSet<usize> {
    let mut interpreter = Interpreter::new(context, candidates);
    let environment = environment();
    interpreter
        .stack
        .frames
        .push(Frame::new(environment.clone(), false, context.root.start_byte()));
    let exit = interpreter.run_frame(context.root, State::default());
    interpreter.stack.frames.clear();
    interpreter.run_stragglers(&environment, &exit);
    interpreter
        .records
        .into_iter()
        .filter(|(_, recorded)| recorded.is_clean())
        .map(|(byte, _)| byte)
        .collect()
}

fn environment() -> State {
    let mut environment = State::default();
    for name in INTERNAL_VARIABLES {
        environment.insert(Scope::Global, name, Variable::UNKNOWN);
    }
    for name in WITHOUT_SPACES {
        environment.insert(Scope::Global, name, Variable::CLEAN);
    }
    for name in SPECIAL_INTEGER_VARIABLES {
        environment.insert(Scope::Global, name, Variable::INTEGER_VALUE);
    }
    environment
}

pub struct Interpreter<'tree, 'context, 'source> {
    context: &'context Context<'tree, 'source>,
    stack: Stack,
    wanted: HashMap<usize, String>,
    records: HashMap<usize, Variable>,
    functions: HashMap<usize, Node<'tree>>,
    invoked: HashSet<usize>,
    returns: Vec<Vec<State>>,
    in_function: bool,
    lastpipe: bool,
    memo: HashMap<usize, Vec<(Vec<Dependency>, State)>>,
}

impl<'tree, 'context, 'source> Interpreter<'tree, 'context, 'source> {
    fn new(context: &'context Context<'tree, 'source>, candidates: &[Node<'tree>]) -> Self {
        let wanted = candidates
            .iter()
            .map(|node| {
                (
                    node.start_byte(),
                    reference(&braced_content(*node, context.source)),
                )
            })
            .collect();
        let functions = descendants(context.root)
            .into_iter()
            .filter(|node| node.kind() == FUNCTION)
            .map(|node| (node.start_byte(), node))
            .collect();
        Self {
            context,
            stack: Stack::default(),
            wanted,
            records: HashMap::new(),
            functions,
            invoked: HashSet::new(),
            returns: Vec::new(),
            in_function: false,
            lastpipe: has_lastpipe(context),
            memo: HashMap::new(),
        }
    }

    pub const fn source(&self) -> &'source [u8] {
        self.context.source
    }

    pub fn record(&mut self, expansion: Node<'_>, state: &State) {
        if state.reach == Reach::Dead {
            return;
        }
        let Some(name) = self.wanted.get(&expansion.start_byte()) else {
            return;
        };
        let (found, _) = self.stack.lookup(state, name);
        self.records
            .entry(expansion.start_byte())
            .and_modify(|recorded| *recorded = recorded.merge(found))
            .or_insert(found);
    }

    fn run_frame(&mut self, body: Node<'tree>, state: State) -> State {
        self.returns.push(Vec::new());
        let end = self.statements(&[body], state);
        let mut exits = self.returns.pop().unwrap_or_default();
        exits.push(end);
        self.stack.merge(exits)
    }

    pub fn subshell(&mut self, body: &[Node<'tree>], state: &State) {
        self.enter_subshell(body.first().map_or(0, Node::start_byte), state);
        self.statements(body, State::default());
        self.leave_subshell();
    }

    pub fn isolated(&mut self, statement: Node<'tree>, state: &State) {
        self.enter_subshell(statement.start_byte(), state);
        self.statement(statement, State::default());
        self.leave_subshell();
    }

    fn enter_subshell(&mut self, entry: usize, state: &State) {
        self.stack.frames.push(Frame::new(state.clone(), false, entry));
        self.returns.push(Vec::new());
    }

    fn leave_subshell(&mut self) {
        self.returns.pop();
        self.stack.frames.pop();
    }

    pub fn declare_function(&self, node: Node<'_>, state: &mut State) {
        if let Some(name) = node.child_by_field_name(NAME_FIELD) {
            state.define(
                text(name, self.source()),
                BTreeSet::from([Target::Defined(node.start_byte())]),
            );
        }
    }
}

fn has_lastpipe(context: &Context<'_, '_>) -> bool {
    match context.dialect {
        Dialect::Ksh => true,
        Dialect::Sh | Dialect::Dash | Dialect::Busybox => false,
        Dialect::Bash => descendants(context.root)
            .into_iter()
            .filter_map(Command::of)
            .any(|command| sets_lastpipe(&command, context.source)),
    }
}

fn sets_lastpipe(command: &Command<'_>, source: &[u8]) -> bool {
    let mentions = || {
        command
            .words
            .iter()
            .any(|word| oversimplify(*word, source) == LASTPIPE)
    };
    (command.is_exactly(SHOPT, source) && mentions())
        || (command.is_exactly(SET, source) && (mentions() || command.has_flag(SET_OPTION_FLAG, source)))
}
