use std::collections::HashMap;

use crate::lint::go::ast::IdentId;

pub type ObjectId = usize;
pub type ScopeId = usize;

const BLANK: &str = "_";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectKind {
    Var,
    Const,
    Type,
    Func,
    Label,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Object {
    pub kind: ObjectKind,
    pub hidden: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Binding {
    Free,
    Unresolved,
    Bound(ObjectId),
}

#[derive(Debug, Default)]
struct Scope {
    outer: Option<ScopeId>,
    entries: HashMap<String, ObjectId>,
}

#[derive(Debug, Default)]
pub struct Scopes {
    scopes: Vec<Scope>,
    objects: Vec<Object>,
}

impl Scopes {
    pub fn open(&mut self, outer: Option<ScopeId>) -> ScopeId {
        self.scopes.push(Scope {
            outer,
            entries: HashMap::new(),
        });
        self.scopes.len().saturating_sub(1)
    }

    pub fn outer(&self, scope: ScopeId) -> Option<ScopeId> {
        self.scopes.get(scope).and_then(|scope| scope.outer)
    }

    pub fn lookup(&self, scope: ScopeId, name: &str) -> Option<ObjectId> {
        self.scopes
            .get(scope)
            .and_then(|scope| scope.entries.get(name).copied())
    }

    pub fn lookup_chain(&self, innermost: Option<ScopeId>, name: &str) -> Option<ObjectId> {
        std::iter::successors(innermost, |scope| self.outer(*scope))
            .find_map(|scope| self.lookup(scope, name))
    }

    pub fn new_object(&mut self, kind: ObjectKind, hidden: bool) -> ObjectId {
        self.objects.push(Object { kind, hidden });
        self.objects.len().saturating_sub(1)
    }

    pub fn is_hidden(&self, object: ObjectId) -> bool {
        self.objects.get(object).is_some_and(|object| object.hidden)
    }

    pub fn insert(&mut self, scope: ScopeId, name: &str, object: ObjectId) -> Option<ObjectId> {
        if name == BLANK {
            return None;
        }
        let entries = &mut self.scopes.get_mut(scope)?.entries;
        let existing = entries.get(name).copied();
        if existing.is_none() {
            entries.insert(name.to_owned(), object);
        }
        existing
    }
}

#[derive(Debug, Default)]
pub struct Labels {
    scopes: Vec<HashMap<String, ObjectId>>,
    targets: Vec<Vec<IdentId>>,
}

impl Labels {
    pub fn open(&mut self) {
        self.scopes.push(HashMap::new());
        self.targets.push(Vec::new());
    }

    pub fn close<'name>(
        &mut self,
        name_of: impl Fn(IdentId) -> &'name str,
    ) -> Vec<(IdentId, Option<ObjectId>)> {
        let scope = self.scopes.pop().unwrap_or_default();
        self.targets
            .pop()
            .unwrap_or_default()
            .into_iter()
            .map(|target| (target, scope.get(name_of(target)).copied()))
            .collect()
    }

    pub fn declare(&mut self, name: &str, object: ObjectId) {
        if name == BLANK {
            return;
        }
        if let Some(scope) = self.scopes.last_mut() {
            scope.entry(name.to_owned()).or_insert(object);
        }
    }

    pub fn target(&mut self, label: IdentId) {
        if let Some(targets) = self.targets.last_mut() {
            targets.push(label);
        }
    }
}
