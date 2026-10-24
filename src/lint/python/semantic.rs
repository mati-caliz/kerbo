mod aliases;
mod annotations;
mod boundary;
mod builtin_calls;
mod cached_methods;
mod calls;
mod checks;
mod classes;
mod collection_calls;
mod comparisons;
mod conversions;
mod decimals;
mod defaults;
mod definitions;
mod effects;
mod expressions;
mod format_calls;
mod handlers;
mod idioms;
mod implicit_optional;
mod imports;
mod integer_calls;
mod isinstance_calls;
mod literals;
mod model;
mod mutability;
mod patterns;
mod pep;
mod qualified;
mod resources;
mod returns;
mod statements;
mod supers;
mod suspicious_calls;
mod targets;
mod type_names;
mod typecheck;

use crate::lint::python::semantic::annotations::Annotations;
use crate::lint::python::semantic::checks::Exports;
use crate::lint::python::semantic::definitions::Definitions;
use crate::lint::python::semantic::expressions::Expressions;
use crate::lint::python::semantic::statements::Statements;
use tree_sitter::{Node, Point};

use super::{FileKind, Report};
pub use checks::is_dummy;
use model::{BranchId, Model, ScopeId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Value,
    TypeDefinition,
    Literal,
}

impl Mode {
    pub const fn defines_types(self) -> bool {
        matches!(self, Self::TypeDefinition)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Pending<'tree> {
    pub node: Node<'tree>,
    pub scope: ScopeId,
    pub branch: Option<BranchId>,
}

#[derive(Debug, Default)]
pub struct Deferred<'tree> {
    pub functions: Vec<Pending<'tree>>,
    pub lambdas: Vec<Pending<'tree>>,
    pub annotations: Vec<Pending<'tree>>,
    pub strings: Vec<Pending<'tree>>,
}

impl Deferred<'_> {
    const fn is_empty(&self) -> bool {
        self.functions.is_empty()
            && self.lambdas.is_empty()
            && self.annotations.is_empty()
            && self.strings.is_empty()
    }
}

#[derive(Debug)]
pub struct Visitor<'tree> {
    pub model: Model<'tree>,
    pub source: &'tree [u8],
    pub file: FileKind,
    pub reports: Vec<Report>,
    pub deferred: Deferred<'tree>,
    pub boundaries: boundary::Boundaries,
    pub exports: Vec<(String, Point)>,
    pub classes: Vec<Pending<'tree>>,
}

pub fn reports(root: Node<'_>, source: &[u8], file: FileKind) -> Vec<Report> {
    let mut visitor = Visitor {
        model: Model::new(),
        source,
        file,
        reports: Vec::new(),
        deferred: Deferred::default(),
        boundaries: boundary::Boundaries::default(),
        exports: Vec::new(),
        classes: Vec::new(),
    };
    visitor.block(root);
    visitor.run_deferred();
    visitor.visit_exports();
    let mut reports = classes::mutable_class_defaults(&visitor.model, source, &visitor.classes);
    reports.extend(checks::reports(&visitor));
    reports.append(&mut visitor.reports);
    reports
}

impl<'tree> Visitor<'tree> {
    pub fn text(&self, node: Node<'_>) -> &'tree str {
        super::ast::text(node, self.source)
    }

    pub const fn restore(&mut self, pending: Pending<'tree>) {
        self.model.scope = pending.scope;
        self.model.branch = pending.branch;
    }

    pub const fn snapshot(&self, node: Node<'tree>) -> Pending<'tree> {
        Pending {
            node,
            scope: self.model.scope,
            branch: self.model.branch,
        }
    }

    fn run_deferred(&mut self) {
        while !self.deferred.is_empty() {
            while let Some(function) = take_first(&mut self.deferred.functions) {
                self.function_body(function);
            }
            while let Some(lambda) = take_first(&mut self.deferred.lambdas) {
                self.lambda_body(lambda);
            }
            self.model.forward_reference = true;
            while let Some(annotation) = take_first(&mut self.deferred.annotations) {
                self.restore(annotation);
                self.expression(annotation.node, Mode::TypeDefinition);
            }
            while let Some(string) = take_first(&mut self.deferred.strings) {
                self.string_annotation(string);
            }
            self.model.forward_reference = false;
        }
    }
}

fn take_first<Item>(queue: &mut Vec<Item>) -> Option<Item> {
    (!queue.is_empty()).then(|| queue.remove(0))
}
