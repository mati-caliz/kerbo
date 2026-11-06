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
use super::syntax::{ASSIGNMENT, EXPANSIONS, FUNCTION, NAME_FIELD, descendants, text};
use super::variables::assigned_name;
use super::words::{braced_content, oversimplify};
use frames::{Dependency, Frame, Stack};
use state::{Reach, Scope, State, Target, Variable};

const LASTPIPE: &str = "lastpipe";
// La entrada de un marco que no es de una función: si fuera un byte del archivo, una función que empieza
// donde empieza el marco parecería estar llamándose a sí misma.
const NOT_A_FUNCTION: usize = usize::MAX;
const SET: &str = "set";
const SHOPT: &str = "shopt";
const SET_OPTION_FLAG: &str = "o";

pub fn clean_expansions(context: &Context<'_, '_>, candidates: &[Node<'_>]) -> HashSet<usize> {
    expansions_where(context, candidates, Variable::is_clean)
}

pub fn integer_assignments(context: &Context<'_, '_>, candidates: &[Node<'_>]) -> HashSet<usize> {
    expansions_where(context, candidates, |variable| variable.integer)
}

pub fn numeric_expansions(context: &Context<'_, '_>, candidates: &[Node<'_>]) -> HashSet<usize> {
    expansions_where(context, candidates, |variable| variable.numeric)
}

// Lo que el recorrido dejó sin alcanzar: las funciones que nada llama y las sentencias de después de un
// `exit` o un `return`, salvo que por otro camino sí se ejecuten.
#[derive(Debug, Default)]
pub struct Reachability<'tree> {
    invoked: HashSet<usize>,
    dead: Vec<Node<'tree>>,
    dead_starts: HashSet<usize>,
}

impl<'tree> Reachability<'tree> {
    pub fn is_invoked(&self, function: Node<'_>) -> bool {
        self.invoked.contains(&function.start_byte())
    }

    pub fn is_dead(&self, statement: Node<'_>) -> bool {
        self.dead_starts.contains(&statement.start_byte())
    }

    pub fn dead_statements(&self) -> &[Node<'tree>] {
        &self.dead
    }
}

pub fn reachability<'tree>(context: &Context<'tree, '_>) -> Reachability<'tree> {
    let mut interpreter = Interpreter::new(context, &[]);
    let environment = environment();
    interpreter
        .stack
        .frames
        .push(Frame::new(environment.clone(), false, NOT_A_FUNCTION));
    let exit = interpreter.run_frame(context.root, State::default());
    interpreter.stack.frames.clear();
    interpreter.run_stragglers(&environment, &exit);
    let live = std::mem::take(&mut interpreter.live_statements);
    let mut dead: Vec<Node<'tree>> = std::mem::take(&mut interpreter.dead_statements)
        .into_values()
        .filter(|statement| !live.contains(&statement.start_byte()))
        .collect();
    dead.sort_by_key(Node::start_byte);
    Reachability {
        invoked: interpreter.invoked,
        dead_starts: dead.iter().map(Node::start_byte).collect(),
        dead,
    }
}

fn expansions_where(
    context: &Context<'_, '_>,
    candidates: &[Node<'_>],
    holds: impl Fn(Variable) -> bool,
) -> HashSet<usize> {
    let mut interpreter = Interpreter::new(context, candidates);
    let environment = environment();
    interpreter
        .stack
        .frames
        .push(Frame::new(environment.clone(), false, NOT_A_FUNCTION));
    let exit = interpreter.run_frame(context.root, State::default());
    interpreter.stack.frames.clear();
    interpreter.run_stragglers(&environment, &exit);
    interpreter
        .records
        .into_iter()
        .filter(|(_, recorded)| holds(*recorded))
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
    live_statements: HashSet<usize>,
    dead_statements: HashMap<usize, Node<'tree>>,
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
                let name = if EXPANSIONS.contains(&node.kind()) {
                    reference(&braced_content(*node, context.source))
                } else if node.kind() == ASSIGNMENT {
                    assigned_name(*node, context.source).unwrap_or_default()
                } else {
                    text(*node, context.source).to_owned()
                };
                (node.start_byte(), name)
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
            live_statements: HashSet::new(),
            dead_statements: HashMap::new(),
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
        self.enter_subshell(state);
        self.statements(body, Self::subshell_start(state));
        self.leave_subshell();
    }

    pub fn isolated(&mut self, statement: Node<'tree>, state: &State) {
        self.enter_subshell(state);
        self.statement(statement, Self::subshell_start(state));
        self.leave_subshell();
    }

    // Un subshell arranca con un estado propio, y si lo de afuera no se alcanza, lo de adentro tampoco.
    fn subshell_start(outer: &State) -> State {
        if outer.is_live() {
            State::default()
        } else {
            State::empty(Reach::Unreachable)
        }
    }

    fn enter_subshell(&mut self, state: &State) {
        self.stack
            .frames
            .push(Frame::new(state.clone(), false, NOT_A_FUNCTION));
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

pub fn has_lastpipe(context: &Context<'_, '_>) -> bool {
    match context.dialect {
        Dialect::Ksh => true,
        Dialect::Sh | Dialect::Dash | Dialect::Busybox => false,
        Dialect::Bash => has_option(context, LASTPIPE),
    }
}

// `isOptionSet`, que sólo mira bash.
pub fn has_option(context: &Context<'_, '_>, option: &str) -> bool {
    context.dialect == Dialect::Bash
        && descendants(context.root)
            .into_iter()
            .filter_map(Command::of)
            .any(|command| sets_option(&command, option, context.source))
}

fn sets_option(command: &Command<'_>, option: &str, source: &[u8]) -> bool {
    let mentions = || {
        command
            .words
            .iter()
            .any(|word| oversimplify(*word, source) == option)
    };
    (command.is_exactly(SHOPT, source) && mentions())
        || (command.is_exactly(SET, source) && (mentions() || command.has_flag(SET_OPTION_FLAG, source)))
}
