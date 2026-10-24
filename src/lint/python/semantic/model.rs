mod resolve;

use std::collections::HashMap;

use tree_sitter::{Node, Point};

pub use resolve::Resolution;

pub type ScopeId = usize;
pub type BindingId = usize;
pub type BranchId = usize;

pub const MODULE_SCOPE: ScopeId = 0;
pub const EXPLICIT_EXPORT: u8 = 1;
pub const GLOBAL: u8 = 2;
pub const NONLOCAL: u8 = 4;
pub const UNPACKED: u8 = 8;
pub const NAME_ERROR: u8 = 1;
const INHERITED_FLAGS: u8 = GLOBAL | NONLOCAL;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    Module,
    Class,
    Function,
    Lambda,
    Generator,
    Type,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingKind {
    Annotation,
    Argument,
    NamedExpression,
    Assignment,
    TypeParameter,
    LoopVariable,
    WithItem,
    Global(Option<BindingId>),
    Nonlocal(Option<BindingId>),
    Class,
    Function,
    Export,
    FutureImport,
    Import,
    FromImport,
    SubmoduleImport,
    Deletion,
    BoundException,
    UnboundException(Option<BindingId>),
}

impl BindingKind {
    pub const fn is_import(self) -> bool {
        matches!(self, Self::Import | Self::FromImport | Self::SubmoduleImport)
    }

    pub const fn is_unbound(self) -> bool {
        matches!(
            self,
            Self::Annotation | Self::Deletion | Self::UnboundException(_)
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reference {
    pub forward: bool,
    pub scope: ScopeId,
    pub at: Point,
}

#[derive(Debug, Clone, Copy)]
pub struct Site<'tree> {
    pub at: Point,
    pub node: Option<Node<'tree>>,
    pub statement: Option<Node<'tree>>,
}

#[derive(Debug, Clone)]
pub struct Binding<'tree> {
    pub kind: BindingKind,
    pub site: Site<'tree>,
    pub scope: ScopeId,
    pub branch: Option<BranchId>,
    pub flags: u8,
    pub references: Vec<Reference>,
    pub qualified: String,
}

impl Binding<'_> {
    pub const fn has(&self, flag: u8) -> bool {
        self.flags & flag != 0
    }

    pub const fn is_used(&self) -> bool {
        !self.references.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct Scope {
    pub kind: ScopeKind,
    pub parent: Option<ScopeId>,
    pub names: HashMap<String, BindingId>,
    pub shadowed: HashMap<BindingId, BindingId>,
    pub star_import: bool,
    pub uses_locals: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unresolved {
    pub name: String,
    pub at: Point,
    pub wildcard: bool,
    pub handled: u8,
}

#[derive(Debug)]
pub struct Model<'tree> {
    pub scopes: Vec<Scope>,
    pub bindings: Vec<Binding<'tree>>,
    pub cross_shadowed: HashMap<BindingId, BindingId>,
    pub unresolved: Vec<Unresolved>,
    pub scope: ScopeId,
    pub branch: Option<BranchId>,
    pub handled: Vec<u8>,
    pub forward_reference: bool,
    pub branches: Vec<Option<BranchId>>,
}

impl<'tree> Model<'tree> {
    pub fn new() -> Self {
        Self {
            scopes: vec![new_scope(ScopeKind::Module, None)],
            bindings: Vec::new(),
            cross_shadowed: HashMap::new(),
            unresolved: Vec::new(),
            scope: MODULE_SCOPE,
            branch: None,
            handled: Vec::new(),
            forward_reference: false,
            branches: Vec::new(),
        }
    }

    pub fn push_scope(&mut self, kind: ScopeKind) -> ScopeId {
        self.scopes.push(new_scope(kind, Some(self.scope)));
        self.scope = self.scopes.len().saturating_sub(1);
        self.scope
    }

    pub fn pop_scope(&mut self) {
        self.scope = self.current().parent.unwrap_or(MODULE_SCOPE);
    }

    pub fn current(&self) -> &Scope {
        self.scope_of(self.scope)
    }

    pub fn scope_of(&self, scope: ScopeId) -> &Scope {
        self.scopes.get(scope).unwrap_or_else(|| empty_scope())
    }

    pub fn current_mut(&mut self) -> Option<&mut Scope> {
        self.scopes.get_mut(self.scope)
    }

    pub fn binding(&self, id: BindingId) -> Option<&Binding<'tree>> {
        self.bindings.get(id)
    }

    pub fn push_branch(&mut self) {
        self.branches.push(self.branch);
        self.branch = Some(self.branches.len().saturating_sub(1));
    }

    pub fn parent_branch(&self) -> Option<BranchId> {
        self.branch
            .and_then(|branch| self.branches.get(branch).copied().flatten())
    }

    pub fn pop_branch(&mut self) {
        self.branch = self
            .branch
            .and_then(|branch| self.branches.get(branch).copied().flatten());
    }

    pub fn same_branch(left: Option<BranchId>, right: Option<BranchId>) -> bool {
        left == right
    }

    pub fn handled_exceptions(&self) -> u8 {
        self.handled.iter().fold(0, |all, flags| all | flags)
    }

    pub fn ancestors(&self, scope: ScopeId) -> impl Iterator<Item = ScopeId> + '_ {
        std::iter::successors(Some(scope), |current| self.scope_of(*current).parent)
    }

    pub fn add_binding(&mut self, name: &str, kind: BindingKind, site: Site<'tree>) -> BindingId {
        let scope = self.binding_scope(kind);
        let id = self.push_binding(kind, site);
        if let Some(binding) = self.bindings.get_mut(id) {
            binding.scope = scope;
        }
        let existing = self.scope_of(scope).names.get(name).copied();
        if let Some(existing) = existing {
            if kind == BindingKind::Annotation {
                return id;
            }
            self.inherit(id, existing);
        } else if self.scope_of(scope).kind != ScopeKind::Class
            && let Some(outer) = self.outer_binding(scope, name)
        {
            self.cross_shadowed.insert(id, outer);
        }
        self.insert(scope, name, id);
        id
    }

    pub fn insert(&mut self, scope: ScopeId, name: &str, id: BindingId) {
        if let Some(target) = self.scopes.get_mut(scope)
            && let Some(previous) = target.names.insert(name.to_owned(), id)
        {
            target.shadowed.insert(id, previous);
        }
    }

    pub fn push_binding(&mut self, kind: BindingKind, site: Site<'tree>) -> BindingId {
        let id = self.bindings.len();
        self.bindings.push(Binding {
            kind,
            site,
            scope: self.scope,
            branch: self.branch,
            flags: 0,
            references: Vec::new(),
            qualified: String::new(),
        });
        id
    }

    pub fn flag(&mut self, id: BindingId, flags: u8) {
        if let Some(binding) = self.bindings.get_mut(id) {
            binding.flags |= flags;
        }
    }

    fn binding_scope(&self, kind: BindingKind) -> ScopeId {
        if kind != BindingKind::NamedExpression {
            return self.scope;
        }
        let mut chosen = self.scope;
        for scope in self.ancestors(self.scope) {
            chosen = scope;
            if self.scope_of(scope).kind != ScopeKind::Generator {
                break;
            }
        }
        chosen
    }

    fn inherit(&mut self, id: BindingId, existing: BindingId) {
        let Some(previous) = self.bindings.get(existing) else {
            return;
        };
        if matches!(
            previous.kind,
            BindingKind::Deletion | BindingKind::UnboundException(_)
        ) {
            return;
        }
        let references = previous.references.clone();
        let inherited = previous.flags & INHERITED_FLAGS;
        if let Some(binding) = self.bindings.get_mut(id) {
            binding.references = references;
            binding.flags |= inherited;
        }
    }

    // Los alcances de clase no se ven desde adentro de sus funciones, así que no cuentan.
    fn outer_binding(&self, scope: ScopeId, name: &str) -> Option<BindingId> {
        self.ancestors(scope)
            .skip(1)
            .filter(|outer| self.scope_of(*outer).kind != ScopeKind::Class)
            .find_map(|outer| self.scope_of(outer).names.get(name).copied())
    }

    pub fn reference(&mut self, id: BindingId, at: Point) {
        let scope = self.scope;
        let forward = self.forward_reference;
        if let Some(binding) = self.bindings.get_mut(id) {
            binding.references.push(Reference { forward, scope, at });
        }
    }

    pub fn shadowed_in_scope(&self, scope: ScopeId, id: BindingId) -> Option<BindingId> {
        self.scope_of(scope).shadowed.get(&id).copied()
    }
}

fn new_scope(kind: ScopeKind, parent: Option<ScopeId>) -> Scope {
    Scope {
        kind,
        parent,
        names: HashMap::new(),
        shadowed: HashMap::new(),
        star_import: false,
        uses_locals: false,
    }
}

fn empty_scope() -> &'static Scope {
    static EMPTY: std::sync::OnceLock<Scope> = std::sync::OnceLock::new();
    EMPTY.get_or_init(|| new_scope(ScopeKind::Module, None))
}
