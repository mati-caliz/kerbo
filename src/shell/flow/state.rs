use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

const SPACE_CHARACTERS: &[char] = &[' ', '\t', '\n', '*', '?', '['];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Space {
    Empty,
    Clean,
    Dirty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Variable {
    pub space: Space,
    pub integer: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Target {
    Unknown,
    Defined(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Scope {
    Global,
    Local,
    Prefix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Reach {
    Live,
    Unreachable,
    Dead,
}

pub type Variables = Rc<BTreeMap<String, Variable>>;
pub type Functions = Rc<BTreeMap<String, BTreeSet<Target>>>;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct State {
    pub globals: Variables,
    pub locals: Variables,
    pub prefix: Variables,
    pub functions: Functions,
    pub reach: Reach,
}

impl Default for State {
    fn default() -> Self {
        Self::empty(Reach::Live)
    }
}

impl Variable {
    pub const UNKNOWN: Self = Self {
        space: Space::Dirty,
        integer: false,
    };
    pub const UNSET: Self = Self {
        space: Space::Empty,
        integer: false,
    };
    pub const INTEGER_VALUE: Self = Self {
        space: Space::Clean,
        integer: false,
    };
    pub const CLEAN: Self = Self::INTEGER_VALUE;

    pub fn literal(text: &str) -> Self {
        let space = if text.is_empty() {
            Space::Empty
        } else if text.contains(SPACE_CHARACTERS) {
            Space::Dirty
        } else {
            Space::Clean
        };
        Self {
            space,
            integer: false,
        }
    }

    pub const fn is_clean(self) -> bool {
        self.integer || matches!(self.space, Space::Clean)
    }

    pub const fn merge(self, other: Self) -> Self {
        Self {
            space: Self::combined(self.space, other.space),
            integer: self.integer && other.integer,
        }
    }

    pub const fn append(self, other: Self) -> Self {
        Self {
            space: Self::combined(self.space, other.space),
            integer: false,
        }
    }

    const fn combined(left: Space, right: Space) -> Space {
        match (left, right) {
            (Space::Empty, space) | (space, Space::Empty) => space,
            (Space::Clean, Space::Clean) => Space::Clean,
            _ => Space::Dirty,
        }
    }

    pub const fn with_value(self, value: Self) -> Self {
        Self {
            space: value.space,
            integer: self.integer,
        }
    }

    pub const fn as_value(self) -> Self {
        if self.integer {
            Self::INTEGER_VALUE
        } else {
            Self {
                space: self.space,
                integer: false,
            }
        }
    }
}

impl State {
    pub fn empty(reach: Reach) -> Self {
        Self {
            globals: Rc::default(),
            locals: Rc::default(),
            prefix: Rc::default(),
            functions: Rc::default(),
            reach,
        }
    }

    pub fn is_live(&self) -> bool {
        self.reach == Reach::Live
    }

    pub fn find(&self, name: &str) -> Option<(Variable, Scope)> {
        self.prefix
            .get(name)
            .map(|found| (*found, Scope::Prefix))
            .or_else(|| self.locals.get(name).map(|found| (*found, Scope::Local)))
            .or_else(|| self.globals.get(name).map(|found| (*found, Scope::Global)))
    }

    pub fn insert(&mut self, scope: Scope, name: &str, variable: Variable) {
        let map = match scope {
            Scope::Global => &mut self.globals,
            Scope::Local => &mut self.locals,
            Scope::Prefix => &mut self.prefix,
        };
        if map.get(name) != Some(&variable) {
            Rc::make_mut(map).insert(name.to_owned(), variable);
        }
    }

    pub fn define(&mut self, name: &str, targets: BTreeSet<Target>) {
        Rc::make_mut(&mut self.functions).insert(name.to_owned(), targets);
    }

    pub fn clear_prefix(&mut self) {
        if !self.prefix.is_empty() {
            self.prefix = Rc::default();
        }
    }

    pub fn clear_locals(&mut self) {
        if !self.locals.is_empty() {
            self.locals = Rc::default();
        }
    }

    pub fn patch(&mut self, diff: &Self) {
        patch_map(&mut self.globals, &diff.globals);
        patch_map(&mut self.locals, &diff.locals);
        patch_map(&mut self.prefix, &diff.prefix);
        if !diff.functions.is_empty() && !Rc::ptr_eq(&self.functions, &diff.functions) {
            let functions = Rc::make_mut(&mut self.functions);
            functions.extend(
                diff.functions
                    .iter()
                    .map(|(name, value)| (name.clone(), value.clone())),
            );
        }
        if diff.reach != Reach::Live {
            self.reach = diff.reach;
        }
    }
}

fn patch_map(base: &mut Variables, diff: &Variables) {
    if diff.is_empty() || Rc::ptr_eq(base, diff) {
        return;
    }
    if base.is_empty() {
        *base = Rc::clone(diff);
        return;
    }
    Rc::make_mut(base).extend(diff.iter().map(|(name, value)| (name.clone(), *value)));
}
