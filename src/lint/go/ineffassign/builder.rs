use std::collections::HashMap;

use super::graph::{BlockId, BranchStack, BranchTarget, Graph, Operation, Variable};
use super::statements;
use crate::lint::go::ast::{
    DeclKind, Expr, Field, File, FuncType, GenDecl, IdentId, Spec, Stmt, UnaryOp, Visitor, walk_expr,
    walk_field_list, walk_func_type, walk_stmt, walk_stmts,
};
use crate::lint::go::resolver::{ObjectId, Resolution};

const BLANK: &str = "_";
const SINGLE_INDEX: usize = 1;

#[derive(Debug)]
pub struct Builder<'file> {
    pub file: &'file File,
    pub resolution: &'file Resolution,
    pub graph: Graph,
    pub roots: Vec<BlockId>,
    pub current: Option<BlockId>,
    pub variables: HashMap<ObjectId, Variable>,
    pub results: Vec<Option<&'file [Field]>>,
    pub defers: Vec<bool>,
    pub breaks: BranchStack,
    pub continues: BranchStack,
    pub gotos: BranchStack,
    pub label_statement: Option<(&'file Stmt, Option<ObjectId>)>,
}

impl<'file> Builder<'file> {
    pub fn function(&mut self, receiver: Option<&'file [Field]>, typ: &'file FuncType, body: &'file [Stmt]) {
        self.shift_function_depth(1);
        self.results.push(typ.results.as_deref());
        self.defers.push(false);
        let saved = self.current;
        let root = self.new_block(&[]);
        self.roots.push(root);
        if let Some(receiver) = receiver {
            walk_field_list(self, receiver);
        }
        walk_func_type(self, typ);
        walk_stmts(self, body);
        self.current = saved;
        self.results.pop();
        self.defers.pop();
        self.shift_function_depth(-1);
    }

    fn shift_function_depth(&mut self, delta: i64) {
        for variable in self.variables.values_mut() {
            variable.function_depth = variable.function_depth.saturating_add(delta);
        }
    }

    pub fn var_decl(&mut self, decl: &'file GenDecl) {
        if decl.kind != DeclKind::Var {
            return;
        }
        for spec in &decl.specs {
            let Spec::Value { names, values, .. } = spec else {
                continue;
            };
            for value in values {
                walk_expr(self, value);
            }
            self.declare_names(names, !values.is_empty());
        }
    }

    fn declare_names(&mut self, names: &[IdentId], initialized: bool) {
        for name in names {
            self.record(*name, initialized);
        }
    }

    pub fn new_block(&mut self, parents: &[BlockId]) -> BlockId {
        let block = self.graph.new_block(parents);
        self.current = Some(block);
        block
    }

    pub fn current_parents(&self) -> Vec<BlockId> {
        self.current.into_iter().collect()
    }

    pub fn statement_label(&self, statement: &Stmt) -> Option<ObjectId> {
        self.label_statement
            .filter(|(labeled, _)| std::ptr::eq(*labeled, statement))
            .and_then(|(_, label)| label)
    }

    pub fn branch_target(&self, label: Option<IdentId>) -> BranchTarget {
        label.map_or(BranchTarget::Innermost, |label| {
            BranchTarget::Labeled(self.resolution.object(label))
        })
    }

    pub fn use_ident(&mut self, ident: IdentId) {
        self.record(ident, false);
    }

    pub fn assign_ident(&mut self, ident: IdentId) {
        self.record(ident, true);
    }

    fn record(&mut self, ident: IdentId, assign: bool) {
        if self.file.name(ident) == BLANK {
            return;
        }
        let Some(object) = self.resolution.object(ident) else {
            return;
        };
        let outside_function = self.current.is_none();
        let variable = self.variables.entry(object).or_default();
        variable.escapes = variable.escapes || variable.function_depth > 0 || outside_function;
        let escapes = variable.escapes;
        if let (Some(block), false) = (self.current, escapes) {
            self.graph.record(block, object, Operation { ident, assign });
        }
    }

    fn escape(&mut self, ident: IdentId) {
        let Some(object) = self.resolution.object(ident) else {
            return;
        };
        if let Some(variable) = self.variables.get_mut(&object) {
            variable.escapes = true;
        }
    }

    pub fn maybe_panic(&mut self) {
        if !self.defers.last().copied().unwrap_or_default() {
            return;
        }
        let Some(Some(results)) = self.results.last().copied() else {
            return;
        };
        for field in results {
            for name in &field.names {
                self.use_ident(*name);
            }
        }
    }

    pub fn walk_optional(&mut self, statement: Option<&'file Stmt>) {
        if let Some(statement) = statement {
            walk_stmt(self, statement);
        }
    }

    fn escaping(expr: &Expr) -> Option<IdentId> {
        match expr {
            Expr::Unary {
                op: UnaryOp::Address,
                operand,
            } => Self::address_target(operand),
            Expr::Selector { operand, .. } | Expr::Slice { operand, .. } => operand.unparen_ident(),
            Expr::Ident(_)
            | Expr::BasicLit(_)
            | Expr::CompositeLit { .. }
            | Expr::KeyValue { .. }
            | Expr::FuncLit { .. }
            | Expr::Paren(_)
            | Expr::Index { .. }
            | Expr::TypeAssert { .. }
            | Expr::Call { .. }
            | Expr::Star(_)
            | Expr::Unary { .. }
            | Expr::Binary { .. }
            | Expr::Ellipsis(_)
            | Expr::ArrayType { .. }
            | Expr::StructType(_)
            | Expr::FuncType(_)
            | Expr::InterfaceType(_)
            | Expr::MapType { .. }
            | Expr::ChanType(_) => None,
        }
    }

    fn address_target(operand: &Expr) -> Option<IdentId> {
        if let Expr::Index { operand, indices } = operand
            && indices.len() == SINGLE_INDEX
        {
            return operand.unparen_ident();
        }
        operand.unparen_ident()
    }

    const fn may_panic(expr: &Expr) -> bool {
        matches!(
            expr,
            Expr::Binary { may_panic: true, .. }
                | Expr::Call { .. }
                | Expr::Star(_)
                | Expr::TypeAssert { .. }
                | Expr::Selector { .. }
                | Expr::Slice { .. }
        ) || matches!(expr, Expr::Index { indices, .. } if indices.len() == SINGLE_INDEX)
    }
}

impl<'file> Visitor<'file> for Builder<'file> {
    fn visit_expr(&mut self, expr: &'file Expr) -> bool {
        if let Expr::Ident(ident) = expr {
            self.use_ident(*ident);
            return false;
        }
        if let Expr::FuncLit { typ, body } = expr {
            self.function(None, typ, body);
            return false;
        }
        if Self::may_panic(expr) {
            self.maybe_panic();
        }
        if let Some(ident) = Self::escaping(expr) {
            self.escape(ident);
        }
        true
    }

    fn visit_stmt(&mut self, statement: &'file Stmt) -> bool {
        statements::statement(self, statement)
    }

    fn visit_name(&mut self, name: IdentId) {
        self.use_ident(name);
    }
}
