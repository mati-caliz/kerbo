mod aliases;
mod annotations;
mod async_blocking;
mod boolean_chains;
mod boundary;
mod builtin_calls;
mod cached_methods;
mod calls;
mod cast_calls;
mod checks;
mod classes;
mod collection_calls;
mod comparisons;
mod compound_checks;
mod conversions;
mod dataclass_checks;
mod dataclass_defaults;
mod decimals;
mod decorators;
mod defaults;
mod definitions;
mod effects;
mod enum_values;
mod exit_annotations;
mod expressions;
mod file_loops;
mod format_calls;
mod gettext_calls;
mod handlers;
mod idioms;
mod if_blocks;
mod implicit_optional;
mod imports;
mod integer_calls;
mod isinstance_calls;
mod library_calls;
mod literals;
mod logging_calls;
mod misc_calls;
mod model;
mod mutability;
mod parameters;
mod patterns;
mod pep;
mod private_types;
mod pylint_calls;
mod pylint_definitions;
mod pytest_calls;
mod qualified;
mod redundant_tests;
mod resources;
mod returns;
mod ruff_calls;
mod self_types;
mod short_circuit;
mod statements;
mod stub_assignments;
mod stub_conditions;
mod stub_signatures;
mod supers;
mod suspicious_calls;
mod tail_calls;
mod targets;
mod timeouts;
mod type_hints;
mod type_names;
mod type_unions;
mod typecheck;
mod version_checks;

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
    pub methods: Vec<Pending<'tree>>,
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
        methods: Vec::new(),
    };
    visitor.block(root);
    visitor.run_deferred();
    visitor.visit_exports();
    let mut reports = classes::mutable_class_defaults(&visitor.model, source, &visitor.classes);
    reports.extend(checks::reports(&visitor));
    reports.extend(self_types::deferred_type_variables(
        &visitor.model,
        source,
        &visitor.methods,
    ));
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
