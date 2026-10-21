use super::{Expr, Field, FuncType, GenDecl, IdentId, Spec, Stmt};

pub trait Visitor<'ast> {
    fn visit_expr(&mut self, expr: &'ast Expr) -> bool;
    fn visit_stmt(&mut self, stmt: &'ast Stmt) -> bool;
    fn visit_name(&mut self, name: IdentId);
}

pub fn walk_expr<'ast>(visitor: &mut impl Visitor<'ast>, expr: &'ast Expr) {
    if !visitor.visit_expr(expr) {
        return;
    }
    match expr {
        Expr::Ident(_) | Expr::BasicLit(_) => {}
        Expr::CompositeLit { typ, elements } => {
            walk_optional(visitor, typ.as_deref());
            walk_exprs(visitor, elements);
        }
        Expr::KeyValue { key, value } => {
            walk_expr(visitor, key);
            walk_expr(visitor, value);
        }
        Expr::FuncLit { typ, body } => {
            walk_func_type(visitor, typ);
            walk_stmts(visitor, body);
        }
        Expr::Paren(inner) | Expr::Star(inner) | Expr::ChanType(inner) => walk_expr(visitor, inner),
        Expr::Unary { operand, .. } => walk_expr(visitor, operand),
        Expr::Selector { operand, field } => {
            walk_expr(visitor, operand);
            visitor.visit_name(*field);
        }
        Expr::Index { operand, indices }
        | Expr::Slice {
            operand,
            bounds: indices,
        } => {
            walk_expr(visitor, operand);
            walk_exprs(visitor, indices);
        }
        Expr::TypeAssert { operand, typ } => {
            walk_expr(visitor, operand);
            walk_optional(visitor, typ.as_deref());
        }
        Expr::Call { function, arguments } => {
            walk_expr(visitor, function);
            walk_exprs(visitor, arguments);
        }
        Expr::Binary { left, right, .. }
        | Expr::MapType {
            key: left,
            value: right,
        } => {
            walk_expr(visitor, left);
            walk_expr(visitor, right);
        }
        Expr::Ellipsis(element) => walk_optional(visitor, element.as_deref()),
        Expr::ArrayType { length, element } => {
            walk_optional(visitor, length.as_deref());
            walk_expr(visitor, element);
        }
        Expr::StructType(fields) | Expr::InterfaceType(fields) => walk_field_list(visitor, fields),
        Expr::FuncType(typ) => walk_func_type(visitor, typ),
    }
}

pub fn walk_stmt<'ast>(visitor: &mut impl Visitor<'ast>, stmt: &'ast Stmt) {
    if !visitor.visit_stmt(stmt) {
        return;
    }
    match stmt {
        Stmt::Decl(decl) => walk_gen_decl(visitor, decl),
        Stmt::Empty => {}
        Stmt::Labeled { label, statement } => {
            visitor.visit_name(*label);
            walk_stmt(visitor, statement);
        }
        Stmt::Expr(expr) | Stmt::IncDec(expr) | Stmt::Go(expr) | Stmt::Defer(expr) => {
            walk_expr(visitor, expr)
        }
        Stmt::Send { channel, value } => {
            walk_expr(visitor, channel);
            walk_expr(visitor, value);
        }
        Stmt::Assign { left, right, .. } => {
            walk_exprs(visitor, left);
            walk_exprs(visitor, right);
        }
        Stmt::Return(results) => walk_exprs(visitor, results),
        Stmt::Branch { label, .. } => {
            if let Some(label) = label {
                visitor.visit_name(*label);
            }
        }
        Stmt::Block(list) | Stmt::Select(list) => walk_stmts(visitor, list),
        Stmt::If(statement) => {
            walk_optional_stmt(visitor, statement.init.as_ref());
            walk_expr(visitor, &statement.condition);
            walk_stmts(visitor, &statement.body);
            walk_optional_stmt(visitor, statement.alternative.as_ref());
        }
        Stmt::Case { list, body } => {
            walk_exprs(visitor, list.as_deref().unwrap_or_default());
            walk_stmts(visitor, body);
        }
        Stmt::Switch { init, tag, body } => {
            walk_optional_stmt(visitor, init.as_deref());
            if let Some(tag) = tag {
                walk_expr(visitor, tag);
            }
            walk_stmts(visitor, body);
        }
        Stmt::TypeSwitch { init, assign, body } => {
            walk_optional_stmt(visitor, init.as_deref());
            walk_stmt(visitor, assign);
            walk_stmts(visitor, body);
        }
        Stmt::Comm { communication, body } => {
            walk_optional_stmt(visitor, communication.as_deref());
            walk_stmts(visitor, body);
        }
        Stmt::For(statement) => {
            walk_optional_stmt(visitor, statement.init.as_ref());
            if let Some(condition) = &statement.condition {
                walk_expr(visitor, condition);
            }
            walk_optional_stmt(visitor, statement.post.as_ref());
            walk_stmts(visitor, &statement.body);
        }
        Stmt::Range(statement) => {
            for bound in [&statement.key, &statement.value].into_iter().flatten() {
                walk_expr(visitor, bound);
            }
            walk_expr(visitor, &statement.operand);
            walk_stmts(visitor, &statement.body);
        }
    }
}

pub fn walk_field_list<'ast>(visitor: &mut impl Visitor<'ast>, fields: &'ast [Field]) {
    for field in fields {
        for name in &field.names {
            visitor.visit_name(*name);
        }
        if let Some(typ) = &field.typ {
            walk_expr(visitor, typ);
        }
    }
}

pub fn walk_func_type<'ast>(visitor: &mut impl Visitor<'ast>, typ: &'ast FuncType) {
    let lists = typ
        .type_params
        .as_deref()
        .into_iter()
        .chain([typ.params.as_slice()])
        .chain(typ.results.as_deref());
    for list in lists {
        walk_field_list(visitor, list);
    }
}

fn walk_gen_decl<'ast>(visitor: &mut impl Visitor<'ast>, decl: &'ast GenDecl) {
    for spec in &decl.specs {
        match spec {
            Spec::Value { names, typ, values } => {
                for name in names {
                    visitor.visit_name(*name);
                }
                walk_optional(visitor, typ.as_ref());
                walk_exprs(visitor, values);
            }
            Spec::Type {
                name,
                type_params,
                typ,
            } => {
                visitor.visit_name(*name);
                walk_field_list(visitor, type_params.as_deref().unwrap_or_default());
                walk_expr(visitor, typ);
            }
        }
    }
}

fn walk_exprs<'ast>(visitor: &mut impl Visitor<'ast>, exprs: &'ast [Expr]) {
    for expr in exprs {
        walk_expr(visitor, expr);
    }
}

pub fn walk_stmts<'ast>(visitor: &mut impl Visitor<'ast>, stmts: &'ast [Stmt]) {
    for stmt in stmts {
        walk_stmt(visitor, stmt);
    }
}

fn walk_optional<'ast>(visitor: &mut impl Visitor<'ast>, expr: Option<&'ast Expr>) {
    if let Some(expr) = expr {
        walk_expr(visitor, expr);
    }
}

fn walk_optional_stmt<'ast>(visitor: &mut impl Visitor<'ast>, stmt: Option<&'ast Stmt>) {
    if let Some(stmt) = stmt {
        walk_stmt(visitor, stmt);
    }
}
