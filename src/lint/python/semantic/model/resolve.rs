use tree_sitter::Point;

use super::{BindingId, BindingKind, MODULE_SCOPE, Model, ScopeId, ScopeKind, Unresolved};
use crate::lint::python::builtins::is_builtin;

const CLASS_CELL: &str = "__class__";
const CLASS_NAMES: [&str; 2] = ["__module__", "__qualname__"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    Binding(BindingId),
    Builtin,
    Implicit,
    Unresolved,
}

#[derive(Debug, Clone, Copy)]
struct Lookup<'name> {
    name: &'name str,
    at: Point,
}

#[derive(Debug, Clone, Copy)]
enum Step {
    Done(Resolution),
    Skip,
    Next,
}

#[derive(Debug, Clone, Copy, Default)]
struct Walk {
    seen_function: bool,
    starred: bool,
    first: Option<ScopeKind>,
}

impl Walk {
    fn hides_class(self, kind: ScopeKind, index: usize) -> bool {
        let visible = index == 0 || (index == 1 && self.first == Some(ScopeKind::Type));
        kind == ScopeKind::Class && !visible
    }
}

impl Model<'_> {
    pub fn resolve_load(&mut self, name: &str, at: Point) -> Resolution {
        let lookup = Lookup { name, at };
        if let Some(resolution) = self.forward(lookup) {
            return resolution;
        }
        let chain: Vec<ScopeId> = self.ancestors(self.scope).collect();
        let mut walk = Walk {
            first: chain.first().map(|scope| self.scope_of(*scope).kind),
            ..Walk::default()
        };
        for position in chain.into_iter().enumerate() {
            if let Step::Done(resolution) = self.load_step(lookup, position, &mut walk) {
                return resolution;
            }
        }
        self.push_unresolved(name, at, walk.starred);
        Resolution::Unresolved
    }

    fn load_step(&mut self, lookup: Lookup<'_>, (index, scope): (usize, ScopeId), walk: &mut Walk) -> Step {
        let kind = self.scope_of(scope).kind;
        if walk.hides_class(kind, index) {
            return if walk.seen_function && lookup.name == CLASS_CELL {
                Step::Done(Resolution::Implicit)
            } else {
                Step::Skip
            };
        }
        if let Some(id) = self.scope_of(scope).names.get(lookup.name).copied() {
            return self.found(lookup, kind, id).map_or(Step::Skip, Step::Done);
        }
        if let Some(resolution) = implicit(kind, index, lookup.name) {
            return Step::Done(resolution);
        }
        walk.seen_function |= kind == ScopeKind::Function;
        walk.starred |= self.scope_of(scope).star_import;
        Step::Next
    }

    fn forward(&mut self, lookup: Lookup<'_>) -> Option<Resolution> {
        if !self.forward_reference {
            return None;
        }
        let id = self.scope_of(MODULE_SCOPE).names.get(lookup.name).copied()?;
        if self.binding(id).is_none_or(|binding| binding.kind.is_unbound()) {
            return None;
        }
        self.mark(lookup, id);
        Some(Resolution::Binding(id))
    }

    fn found(&mut self, lookup: Lookup<'_>, kind: ScopeKind, id: BindingId) -> Option<Resolution> {
        self.mark(lookup, id);
        match self.binding(id).map(|binding| binding.kind) {
            Some(BindingKind::Annotation) if kind == ScopeKind::Class => None,
            Some(BindingKind::Deletion | BindingKind::UnboundException(None) | BindingKind::Annotation) => {
                self.push_unresolved(lookup.name, lookup.at, false);
                Some(Resolution::Unresolved)
            }
            Some(
                BindingKind::UnboundException(Some(target))
                | BindingKind::Global(Some(target))
                | BindingKind::Nonlocal(Some(target)),
            ) => {
                self.reference(target, lookup.at);
                Some(Resolution::Binding(target))
            }
            _ => Some(Resolution::Binding(id)),
        }
    }

    pub fn lookup(&self, name: &str) -> Resolution {
        self.lookup_from(self.scope, name)
    }

    pub fn lookup_from(&self, start: ScopeId, name: &str) -> Resolution {
        let chain: Vec<ScopeId> = self.ancestors(start).collect();
        let mut walk = Walk {
            first: chain.first().map(|scope| self.scope_of(*scope).kind),
            ..Walk::default()
        };
        for (index, scope) in chain.into_iter().enumerate() {
            let kind = self.scope_of(scope).kind;
            let hidden = walk.hides_class(kind, index);
            if hidden && walk.seen_function && name == CLASS_CELL {
                return Resolution::Unresolved;
            }
            if hidden {
                continue;
            }
            walk.seen_function |= kind == ScopeKind::Function;
            if let Step::Done(resolution) = self.lookup_step(scope, name) {
                return resolution;
            }
        }
        Resolution::Unresolved
    }

    fn lookup_step(&self, scope: ScopeId, name: &str) -> Step {
        let Some(id) = self.scope_of(scope).names.get(name).copied() else {
            let builtin = self.scope_of(scope).kind == ScopeKind::Module && is_builtin(name);
            return if builtin {
                Step::Done(Resolution::Builtin)
            } else {
                Step::Next
            };
        };
        match self.binding(id).map(|binding| binding.kind) {
            Some(BindingKind::Annotation) => Step::Skip,
            Some(BindingKind::Deletion | BindingKind::UnboundException(None)) | None => {
                Step::Done(Resolution::Unresolved)
            }
            Some(BindingKind::UnboundException(Some(target))) => Step::Done(Resolution::Binding(target)),
            Some(_) => Step::Done(Resolution::Binding(id)),
        }
    }

    pub fn resolve_delete(&mut self, name: &str, at: Point) {
        let found = self.current().names.get(name).copied();
        let unbound = found.is_none_or(|id| {
            self.reference(id, at);
            self.binding(id).is_some_and(|binding| binding.kind.is_unbound())
        });
        if unbound {
            self.push_unresolved(name, at, false);
        }
    }

    fn mark(&mut self, lookup: Lookup<'_>, id: BindingId) {
        self.reference(id, lookup.at);
        let Some(binding) = self
            .binding(id)
            .filter(|binding| binding.kind == BindingKind::Import)
        else {
            return;
        };
        let scope = binding.scope;
        let Some(module) = binding
            .qualified
            .split('.')
            .next()
            .filter(|module| *module != lookup.name)
        else {
            return;
        };
        let submodule = self
            .scope_of(scope)
            .names
            .get(module)
            .copied()
            .filter(|submodule| {
                self.binding(*submodule)
                    .is_some_and(|found| found.kind == BindingKind::SubmoduleImport)
            });
        if let Some(submodule) = submodule {
            self.reference(submodule, lookup.at);
        }
    }

    pub fn push_unresolved(&mut self, name: &str, at: Point, wildcard: bool) {
        let handled = self.handled_exceptions();
        self.unresolved.push(Unresolved {
            name: name.to_owned(),
            at,
            wildcard,
            handled,
        });
    }
}

fn implicit(kind: ScopeKind, index: usize, name: &str) -> Option<Resolution> {
    if index == 0 && kind == ScopeKind::Class && CLASS_NAMES.contains(&name) {
        return Some(Resolution::Implicit);
    }
    (kind == ScopeKind::Module && is_builtin(name)).then_some(Resolution::Builtin)
}
