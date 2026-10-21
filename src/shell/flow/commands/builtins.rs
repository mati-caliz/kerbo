use crate::shell::flow::commands::{Commands, Invocation};
use std::collections::BTreeSet;

use tree_sitter::Node;

use crate::shell::commands::{bsd_options, generic_options, gnu_options};
use crate::shell::flow::Interpreter;
use crate::shell::flow::state::{Reach, State, Target, Variable};
use crate::shell::parameters::is_variable_name;
use crate::shell::syntax::{VARIABLE_NAME, text};
use crate::shell::variables::let_assignments;
use crate::shell::words::literal;

const EXIT: &str = "exit";
const RETURN: &str = "return";
const UNSET: &str = "unset";
const PRINTF: &str = "printf";
const WAIT: &str = "wait";
const READ: &str = "read";
const LET: &str = "let";
const MAPFILE_COMMANDS: &[&str] = &["mapfile", "readarray"];
const DEFINE_COMMANDS: &[&str] = &[
    "DEFINE_boolean",
    "DEFINE_float",
    "DEFINE_integer",
    "DEFINE_string",
];
const READ_OPTIONS: &str = "sreu:n:N:i:p:a:t:";
const MAPFILE_OPTIONS: &str = "d:n:O:s:u:C:c:t";
const UNSET_OPTIONS: &str = "vfn";
const PRINTF_OPTIONS: &str = "v:";
const VARIABLE_FLAG: &str = "v";
const WAIT_FLAG: &str = "p";
const ARRAY_FLAG: &str = "a";
const FUNCTION_FLAG: &str = "f";
const NAMEREF_FLAG: &str = "n";
const DEFAULT_REPLY: &str = "REPLY";
const DEFAULT_MAPFILE: &str = "MAPFILE";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Undefine {
    Variable,
    Function,
    Both,
}

pub trait Builtins<'tree> {
    fn builtin(&mut self, name: &str, invocation: Invocation<'_, 'tree>, state: &State) -> Option<State>;
    fn unset(&mut self, arguments: &[Node<'tree>], state: State) -> State;
    fn printf_variable(&self, arguments: &[Node<'_>], state: &mut State);
    fn wait_variable(&self, arguments: &[Node<'_>], state: &mut State);
    fn read_variables(&self, arguments: &[Node<'_>], state: &mut State);
    fn mapfile_variable(&self, arguments: &[Node<'_>], state: &mut State);
    fn define_variable(&self, arguments: &[Node<'_>], state: &mut State);
    fn let_variables(&self, arguments: &[Node<'_>], state: &mut State);
}

impl<'tree> Builtins<'tree> for Interpreter<'tree, '_, '_> {
    fn builtin(&mut self, name: &str, invocation: Invocation<'_, 'tree>, state: &State) -> Option<State> {
        let handled = [EXIT, RETURN, UNSET, PRINTF, WAIT, READ, LET].contains(&name)
            || MAPFILE_COMMANDS.contains(&name)
            || DEFINE_COMMANDS.contains(&name);
        if !handled {
            return None;
        }
        let mut expanded = self.expand_words(invocation, state.clone());
        let arguments = invocation.words.get(1..).unwrap_or_default();
        let prefix = invocation.prefix;
        match name {
            EXIT => return Some(State::empty(Reach::Dead)),
            RETURN => return Some(self.finish_frame(expanded)),
            UNSET => expanded = self.unset(arguments, expanded),
            PRINTF => self.printf_variable(arguments, &mut expanded),
            WAIT => self.wait_variable(arguments, &mut expanded),
            READ => self.read_variables(arguments, &mut expanded),
            LET => self.let_variables(arguments, &mut expanded),
            mapfile if MAPFILE_COMMANDS.contains(&mapfile) => self.mapfile_variable(arguments, &mut expanded),
            _ => self.define_variable(arguments, &mut expanded),
        }
        if !prefix.is_empty() {
            expanded.clear_prefix();
        }
        Some(expanded)
    }

    fn unset(&mut self, arguments: &[Node<'tree>], state: State) -> State {
        let source = self.source();
        let parsed = gnu_options(UNSET_OPTIONS, arguments, source);
        let (flags, names): (Vec<String>, Vec<Node<'tree>>) = parsed.map_or_else(
            || (Vec::new(), arguments.to_vec()),
            |options| {
                let flags = options
                    .iter()
                    .map(|(flag, _)| flag.clone())
                    .filter(|flag| !flag.is_empty())
                    .collect();
                let names = options
                    .iter()
                    .filter(|(flag, _)| flag.is_empty())
                    .map(|(_, option)| option.value)
                    .collect();
                (flags, names)
            },
        );
        let has = |flag: &str| flags.iter().any(|found| found == flag);
        let undefine = if has(NAMEREF_FLAG) || has(VARIABLE_FLAG) {
            Undefine::Variable
        } else if has(FUNCTION_FLAG) {
            Undefine::Function
        } else {
            Undefine::Both
        };
        let mut state = state;
        for name in names.into_iter().filter_map(|node| plain_name(node, source)) {
            if undefine != Undefine::Function {
                self.stack.put(&mut state, &name, Variable::UNSET);
            }
            if undefine != Undefine::Variable {
                state.define(&name, BTreeSet::from([Target::Unknown]));
            }
        }
        state
    }

    fn printf_variable(&self, arguments: &[Node<'_>], state: &mut State) {
        let source = self.source();
        let Some(options) = bsd_options(PRINTF_OPTIONS, arguments, source) else {
            return;
        };
        if let Some((_, option)) = options.iter().find(|(flag, _)| flag == VARIABLE_FLAG)
            && let Some(name) = literal(option.value, source)
        {
            self.stack.write(state, &name, Variable::UNKNOWN);
        }
    }

    fn wait_variable(&self, arguments: &[Node<'_>], state: &mut State) {
        let source = self.source();
        let options = generic_options(arguments, source);
        if let Some((_, option)) = options.iter().find(|(flag, _)| flag == WAIT_FLAG)
            && let Some(name) = literal(option.value, source)
        {
            self.stack.write(state, &name, Variable::INTEGER_VALUE);
        }
    }

    fn read_variables(&self, arguments: &[Node<'_>], state: &mut State) {
        let source = self.source();
        let fallback = || {
            let mut trailing: Vec<String> = arguments
                .iter()
                .rev()
                .map_while(|argument| literal(*argument, source))
                .collect();
            trailing.reverse();
            if trailing.is_empty() {
                vec![DEFAULT_REPLY.to_owned()]
            } else {
                trailing
            }
        };
        let parsed = |options: Vec<(String, crate::shell::commands::Opt<'_>)>| {
            options.iter().find(|(flag, _)| flag == ARRAY_FLAG).map_or_else(
                || {
                    options
                        .iter()
                        .filter(|(flag, _)| flag.is_empty())
                        .filter_map(|(_, option)| literal(option.value, source))
                        .collect()
                },
                |(_, option)| literal(option.value, source).into_iter().collect(),
            )
        };
        let names: Vec<String> = gnu_options(READ_OPTIONS, arguments, source).map_or_else(fallback, parsed);
        for name in names {
            self.stack.write(state, &name, Variable::UNKNOWN);
        }
    }

    fn mapfile_variable(&self, arguments: &[Node<'_>], state: &mut State) {
        let source = self.source();
        let from_options = gnu_options(MAPFILE_OPTIONS, arguments, source).and_then(|options| {
            options
                .iter()
                .find(|(flag, _)| flag.is_empty())
                .map(|(_, option)| literal(option.value, source))
        });
        let fallback = || {
            arguments
                .iter()
                .rev()
                .find_map(|argument| literal(*argument, source).filter(|name| is_variable_name(name)))
        };
        let name = from_options.flatten().or_else(fallback);
        let name = name.unwrap_or_else(|| DEFAULT_MAPFILE.to_owned());
        self.stack.write(state, &name, Variable::UNKNOWN);
    }

    fn let_variables(&self, arguments: &[Node<'_>], state: &mut State) {
        for argument in arguments {
            for name in let_assignments(argument.utf8_text(self.source()).unwrap_or_default()) {
                self.stack.write(state, &name, Variable::INTEGER_VALUE);
            }
        }
    }

    fn define_variable(&self, arguments: &[Node<'_>], state: &mut State) {
        let source = self.source();
        if let Some(name) = arguments
            .get(1)
            .and_then(|argument| literal(*argument, source))
            .filter(|name| is_variable_name(name))
        {
            self.stack.write(state, &name, Variable::UNKNOWN);
        }
    }
}

fn plain_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    if node.kind() == VARIABLE_NAME {
        return Some(text(node, source).to_owned());
    }
    literal(node, source)
}
