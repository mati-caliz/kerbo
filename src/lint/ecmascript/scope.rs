pub mod bindings;
mod classes;
pub mod declarations;
mod expressions;
mod model;
pub mod patterns;
mod referencer;
mod types;

use std::collections::HashMap;

use tree_sitter::Node;

pub type ScopeId = usize;
pub type VariableId = usize;
pub type ReferenceId = usize;

pub use model::{Access, Definition, DefinitionKind, Reference, Target, Variable};

pub const GLOBAL: ScopeId = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    Global,
    Module,
    Function,
    FunctionExpressionName,
    Class,
    ClassFieldInitializer,
    ClassStaticBlock,
    Block,
    For,
    Switch,
    Catch,
    With,
    Type,
    FunctionType,
    MappedType,
    ConditionalType,
    Enum,
    Namespace,
}

impl ScopeKind {
    const fn is_variable_scope(self) -> bool {
        matches!(
            self,
            Self::Global
                | Self::Module
                | Self::Function
                | Self::ClassFieldInitializer
                | Self::ClassStaticBlock
                | Self::Namespace
        )
    }
}

#[derive(Debug)]
pub struct Scope<'tree> {
    pub kind: ScopeKind,
    pub block: Node<'tree>,
    pub upper: Option<ScopeId>,
    pub hoisting: ScopeId,
    pub children: Vec<ScopeId>,
    pub variables: Vec<VariableId>,
    pub names: HashMap<String, VariableId>,
    pub through: Vec<ReferenceId>,
    pub pending: Vec<ReferenceId>,
}

impl Scope<'_> {
    pub fn lookup(&self, name: &str) -> Option<VariableId> {
        self.names.get(name).copied()
    }
}

#[derive(Debug)]
pub struct ScopeTree<'tree> {
    pub scopes: Vec<Scope<'tree>>,
    pub variables: Vec<Variable<'tree>>,
    pub references: Vec<Reference<'tree>>,
    pub by_block: HashMap<usize, Vec<ScopeId>>,
    pub declared: HashMap<usize, Vec<VariableId>>,
}

impl<'tree> ScopeTree<'tree> {
    pub fn analyze(root: Node<'tree>, source: &[u8]) -> Self {
        let mut tree = Self {
            scopes: Vec::new(),
            variables: Vec::new(),
            references: Vec::new(),
            by_block: HashMap::new(),
            declared: HashMap::new(),
        };
        referencer::Referencer::new(&mut tree, source).program(root);
        tree
    }

    pub fn variable(&self, id: VariableId) -> Option<&Variable<'tree>> {
        self.variables.get(id)
    }

    pub fn reference(&self, id: ReferenceId) -> Option<&Reference<'tree>> {
        self.references.get(id)
    }

    pub fn scope(&self, id: ScopeId) -> Option<&Scope<'tree>> {
        self.scopes.get(id)
    }

    pub fn references_of(&self, variable: &Variable<'tree>) -> impl Iterator<Item = &Reference<'tree>> {
        variable
            .references
            .iter()
            .filter_map(|id| self.references.get(*id))
    }

    pub fn declared_by(&self, node: Node<'_>) -> &[VariableId] {
        self.declared.get(&node.id()).map_or(&[], Vec::as_slice)
    }

    pub fn variable_scope_of(&self, scope: ScopeId) -> ScopeId {
        self.scopes.get(scope).map_or(GLOBAL, |found| found.hoisting)
    }

    pub fn scope_of(&self, node: Node<'_>) -> ScopeId {
        let mut current = Some(node);
        while let Some(candidate) = current {
            if let Some(&innermost) = self
                .by_block
                .get(&candidate.id())
                .and_then(|scopes| scopes.last())
            {
                return self.skip_function_name_scope(innermost);
            }
            current = candidate.parent();
        }
        GLOBAL
    }

    fn skip_function_name_scope(&self, scope: ScopeId) -> ScopeId {
        self.scopes
            .get(scope)
            .filter(|found| found.kind == ScopeKind::FunctionExpressionName)
            .and_then(|found| found.children.first().copied())
            .unwrap_or(scope)
    }

    pub fn find(&self, from: ScopeId, name: &str) -> Option<VariableId> {
        let mut current = Some(from);
        while let Some(scope) = current.and_then(|id| self.scopes.get(id)) {
            if let Some(&variable) = scope.names.get(name) {
                return Some(variable);
            }
            current = scope.upper;
        }
        None
    }

    pub fn is_inside_block(&self, reference: &Reference<'_>, blocks: &[Node<'_>]) -> bool {
        let mut current = Some(reference.from);
        while let Some(scope) = current.and_then(|id| self.scopes.get(id)) {
            if blocks.iter().any(|block| block.id() == scope.block.id()) {
                return true;
            }
            current = scope.upper;
        }
        false
    }

    fn open(&mut self, kind: ScopeKind, block: Node<'tree>, upper: Option<ScopeId>) -> ScopeId {
        let id = self.scopes.len();
        let variable_scope = if kind.is_variable_scope() {
            id
        } else {
            upper.map_or(id, |upper| self.variable_scope_of(upper))
        };
        self.scopes.push(Scope {
            kind,
            block,
            upper,
            hoisting: variable_scope,
            children: Vec::new(),
            variables: Vec::new(),
            names: HashMap::new(),
            through: Vec::new(),
            pending: Vec::new(),
        });
        if let Some(parent) = upper.and_then(|upper| self.scopes.get_mut(upper)) {
            parent.children.push(id);
        }
        self.by_block.entry(block.id()).or_default().push(id);
        id
    }

    fn define(&mut self, scope: ScopeId, name: &str, definition: Option<Definition<'tree>>) -> VariableId {
        let existing = self
            .scopes
            .get(scope)
            .and_then(|found| found.names.get(name).copied());
        let variable = existing.unwrap_or_else(|| {
            let id = self.variables.len();
            self.variables.push(Variable {
                name: name.to_owned(),
                scope,
                definitions: Vec::new(),
                references: Vec::new(),
                used: false,
            });
            if let Some(found) = self.scopes.get_mut(scope) {
                found.names.insert(name.to_owned(), id);
                found.variables.push(id);
            }
            id
        });
        if let Some(definition) = definition {
            for node in [Some(definition.node), definition.declaration]
                .into_iter()
                .flatten()
            {
                let declared = self.declared.entry(node.id()).or_default();
                let missing = !declared.contains(&variable);
                declared.extend(missing.then_some(variable));
            }
            if let Some(found) = self.variables.get_mut(variable) {
                found.definitions.push(definition);
            }
        }
        variable
    }

    fn reference_from(&mut self, scope: ScopeId, reference: Reference<'tree>) {
        let id = self.references.len();
        self.references.push(reference);
        if let Some(found) = self.scopes.get_mut(scope) {
            found.pending.push(id);
        }
    }

    fn close(&mut self, scope: ScopeId, source: &[u8]) {
        let pending = self
            .scopes
            .get_mut(scope)
            .map(|found| std::mem::take(&mut found.pending))
            .unwrap_or_default();
        let upper = self.scopes.get(scope).and_then(|found| found.upper);
        for id in pending {
            if self.resolves_here(scope, id, source) {
                continue;
            }
            if let Some(found) = self.scopes.get_mut(scope) {
                found.through.push(id);
            }
            if let Some(parent) = upper.and_then(|upper| self.scopes.get_mut(upper)) {
                parent.pending.push(id);
            }
        }
    }

    fn resolves_here(&mut self, scope: ScopeId, id: ReferenceId, source: &[u8]) -> bool {
        let Some(reference) = self.references.get(id).copied() else {
            return false;
        };
        let name = reference.identifier.utf8_text(source).unwrap_or_default();
        let Some(variable) = self
            .scopes
            .get(scope)
            .and_then(|found| found.names.get(name).copied())
        else {
            return false;
        };
        let Some(candidate) = self.variables.get(variable) else {
            return false;
        };
        let matches_kind = (reference.is_type() && candidate.is_type_variable())
            || (reference.is_value() && candidate.is_value_variable());
        if !matches_kind || !self.is_valid_resolution(scope, &reference, variable) {
            return false;
        }
        if let Some(found) = self.variables.get_mut(variable) {
            found.references.push(id);
        }
        if let Some(resolved) = self.references.get_mut(id) {
            resolved.resolved = Some(variable);
        }
        true
    }

    fn is_valid_resolution(&self, scope: ScopeId, reference: &Reference<'_>, variable: VariableId) -> bool {
        let Some(found) = self.scopes.get(scope) else {
            return true;
        };
        if found.kind != ScopeKind::Function {
            return true;
        }
        let body_start = found
            .block
            .child_by_field_name("body")
            .map_or(0, |body| body.start_byte());
        let in_parameters = reference.identifier.start_byte() < body_start;
        let declared_in_body = self.variables.get(variable).is_some_and(|variable| {
            variable.scope == scope
                && variable
                    .definitions
                    .iter()
                    .all(|definition| definition.name.start_byte() >= body_start)
        });
        !(in_parameters && declared_in_body)
    }
}
