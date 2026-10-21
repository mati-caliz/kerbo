use super::Resolver;
use super::scopes::ObjectKind;
use crate::lint::go::ast::{
    AssignOp, BranchKind, DeclKind, Expr, Field, GenDecl, IdentId, RangeStmt, Spec, Stmt, Visitor, walk_expr,
    walk_stmt,
};

impl<'ast> Visitor<'ast> for Resolver<'_> {
    fn visit_expr(&mut self, expr: &'ast Expr) -> bool {
        match expr {
            Expr::Ident(ident) => self.resolve(*ident, true),
            Expr::FuncLit { typ, body } => {
                self.open_scope();
                self.walk_func_type(typ);
                self.walk_body(body);
                self.close_scope();
            }
            Expr::Selector { operand, .. } => walk_expr(self, operand),
            Expr::StructType(fields) => scoped_fields(self, fields, ObjectKind::Var),
            Expr::InterfaceType(fields) => scoped_fields(self, fields, ObjectKind::Func),
            Expr::FuncType(typ) => {
                self.open_scope();
                self.walk_func_type(typ);
                self.close_scope();
            }
            Expr::CompositeLit { typ, elements } => {
                if let Some(typ) = typ {
                    walk_expr(self, typ);
                }
                for element in elements {
                    composite_element(self, element);
                }
            }
            Expr::BasicLit(_)
            | Expr::KeyValue { .. }
            | Expr::Paren(_)
            | Expr::Index { .. }
            | Expr::Slice { .. }
            | Expr::TypeAssert { .. }
            | Expr::Call { .. }
            | Expr::Star(_)
            | Expr::Unary { .. }
            | Expr::Binary { .. }
            | Expr::Ellipsis(_)
            | Expr::ArrayType { .. }
            | Expr::MapType { .. }
            | Expr::ChanType(_) => return true,
        }
        false
    }

    fn visit_stmt(&mut self, stmt: &'ast Stmt) -> bool {
        match stmt {
            Stmt::Labeled { label, statement } => {
                let object = self.scopes.new_object(ObjectKind::Label, false);
                self.bind(*label, super::Binding::Bound(object));
                self.labels.declare(self.file.name(*label), object);
                walk_stmt(self, statement);
            }
            Stmt::Assign { left, op, right } => {
                self.walk_exprs(right);
                if *op == AssignOp::Define {
                    self.short_var_decl(left);
                } else {
                    self.walk_exprs(left);
                }
            }
            Stmt::Branch { kind, label } => {
                if let Some(label) = label.filter(|_| *kind != BranchKind::Fallthrough) {
                    self.labels.target(label);
                }
            }
            Stmt::Block(list) => self.walk_block(list),
            Stmt::If(statement) => {
                self.open_scope();
                walk_optional(self, statement.init.as_ref());
                walk_expr(self, &statement.condition);
                self.walk_block(&statement.body);
                walk_optional(self, statement.alternative.as_ref());
                self.close_scope();
            }
            Stmt::Case { list, body } => {
                self.walk_exprs(list.as_deref().unwrap_or_default());
                self.walk_block(body);
            }
            Stmt::Switch { init, tag, body } => switch(self, init.as_deref(), tag.as_ref(), body),
            Stmt::TypeSwitch { init, assign, body } => type_switch(self, init.as_deref(), assign, body),
            Stmt::Comm { communication, body } => {
                self.open_scope();
                walk_optional(self, communication.as_deref());
                self.walk_stmts(body);
                self.close_scope();
            }
            Stmt::Select(list) => self.walk_stmts(list),
            Stmt::For(statement) => {
                self.open_scope();
                walk_optional(self, statement.init.as_ref());
                if let Some(condition) = &statement.condition {
                    walk_expr(self, condition);
                }
                walk_optional(self, statement.post.as_ref());
                self.walk_block(&statement.body);
                self.close_scope();
            }
            Stmt::Range(statement) => range(self, statement),
            Stmt::Decl(decl) => gen_decl(self, decl),
            Stmt::Empty
            | Stmt::Expr(_)
            | Stmt::Send { .. }
            | Stmt::IncDec(_)
            | Stmt::Go(_)
            | Stmt::Defer(_)
            | Stmt::Return(_) => return true,
        }
        false
    }

    fn visit_name(&mut self, _name: IdentId) {}
}

pub(super) fn gen_decl(resolver: &mut Resolver<'_>, decl: &GenDecl) {
    let mut opened = 0_usize;
    let value_kind = match decl.kind {
        DeclKind::Var => Some(ObjectKind::Var),
        DeclKind::Const => Some(ObjectKind::Const),
        DeclKind::Type | DeclKind::Import => None,
    };
    for spec in &decl.specs {
        if let (Some(kind), Spec::Value { names, typ, values }) = (value_kind, spec) {
            resolver.walk_exprs(values);
            if let Some(typ) = typ {
                walk_expr(resolver, typ);
            }
            resolver.declare(names, resolver.top, kind);
        }
        if let (
            DeclKind::Type,
            Spec::Type {
                name,
                type_params,
                typ,
            },
        ) = (decl.kind, spec)
        {
            resolver.declare(&[*name], resolver.top, ObjectKind::Type);
            if let Some(type_params) = type_params {
                resolver.open_scope();
                opened = opened.saturating_add(1);
                resolver.walk_type_params(type_params);
            }
            walk_expr(resolver, typ);
        }
    }
    for _ in 0..opened {
        resolver.close_scope();
    }
}

fn scoped_fields(resolver: &mut Resolver<'_>, fields: &[Field], kind: ObjectKind) {
    resolver.open_scope();
    resolver.walk_field_list(fields, kind);
    resolver.close_scope();
}

fn composite_element(resolver: &mut Resolver<'_>, element: &Expr) {
    let Expr::KeyValue { key, value } = element else {
        walk_expr(resolver, element);
        return;
    };
    if let Expr::Ident(ident) = key.as_ref() {
        resolver.resolve(*ident, false);
    } else {
        walk_expr(resolver, key);
    }
    walk_expr(resolver, value);
}

fn walk_optional(resolver: &mut Resolver<'_>, stmt: Option<&Stmt>) {
    if let Some(stmt) = stmt {
        walk_stmt(resolver, stmt);
    }
}

fn switch(resolver: &mut Resolver<'_>, init: Option<&Stmt>, tag: Option<&Expr>, body: &[Stmt]) {
    resolver.open_scope();
    walk_optional(resolver, init);
    let extra_scope = tag.is_some() && init.is_some();
    if extra_scope {
        resolver.open_scope();
    }
    if let Some(tag) = tag {
        walk_expr(resolver, tag);
    }
    resolver.walk_stmts(body);
    if extra_scope {
        resolver.close_scope();
    }
    resolver.close_scope();
}

fn type_switch(resolver: &mut Resolver<'_>, init: Option<&Stmt>, assign: &Stmt, body: &[Stmt]) {
    if init.is_some() {
        resolver.open_scope();
        walk_optional(resolver, init);
    }
    resolver.open_scope();
    walk_stmt(resolver, assign);
    resolver.walk_stmts(body);
    resolver.close_scope();
    if init.is_some() {
        resolver.close_scope();
    }
}

fn range(resolver: &mut Resolver<'_>, statement: &RangeStmt) {
    resolver.open_scope();
    walk_expr(resolver, &statement.operand);
    let bounds: Vec<Expr> = [&statement.key, &statement.value]
        .into_iter()
        .flatten()
        .cloned()
        .collect();
    if statement.define {
        for bound in &bounds {
            if bound.unparen_ident().is_none() {
                walk_expr(resolver, bound);
            }
        }
        resolver.short_var_decl(&bounds);
    } else {
        resolver.walk_exprs(&bounds);
    }
    resolver.walk_block(&statement.body);
    resolver.close_scope();
}
