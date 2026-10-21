mod scopes;
mod visit;

use self::scopes::{Binding, Labels, ObjectKind, ScopeId, Scopes};
use super::ast::{Decl, Expr, Field, File, FuncDecl, FuncType, IdentId, Stmt, walk_expr, walk_stmt};

pub use self::scopes::ObjectId;

const PACKAGE_SCOPE: ScopeId = 0;
const BLANK: &str = "_";
const INIT_FUNCTION: &str = "init";

#[derive(Debug)]
pub struct Resolution {
    bindings: Vec<Binding>,
}

impl Resolution {
    pub fn object(&self, ident: IdentId) -> Option<ObjectId> {
        match self.bindings.get(ident) {
            Some(Binding::Bound(object)) => Some(*object),
            Some(Binding::Free | Binding::Unresolved) | None => None,
        }
    }
}

pub fn resolve(file: &File) -> Resolution {
    let mut resolver = Resolver {
        file,
        scopes: Scopes::default(),
        top: None,
        labels: Labels::default(),
        unresolved: Vec::new(),
        bindings: vec![Binding::Free; file.idents.len()],
    };
    resolver.top = Some(resolver.scopes.open(None));
    for decl in &file.decls {
        match decl {
            Decl::Func(function) => resolver.func_decl(function),
            Decl::Gen(decl) => visit::gen_decl(&mut resolver, decl),
        }
    }
    for ident in std::mem::take(&mut resolver.unresolved) {
        let object = resolver.scopes.lookup(PACKAGE_SCOPE, file.name(ident));
        resolver.bind(ident, object.map_or(Binding::Free, Binding::Bound));
    }
    Resolution {
        bindings: resolver.bindings,
    }
}

#[derive(Debug)]
struct Resolver<'file> {
    file: &'file File,
    scopes: Scopes,
    top: Option<ScopeId>,
    labels: Labels,
    unresolved: Vec<IdentId>,
    bindings: Vec<Binding>,
}

impl Resolver<'_> {
    fn bind(&mut self, ident: IdentId, binding: Binding) {
        if let Some(slot) = self.bindings.get_mut(ident) {
            *slot = binding;
        }
    }

    fn open_scope(&mut self) {
        self.top = Some(self.scopes.open(self.top));
    }

    fn close_scope(&mut self) {
        self.top = self.top.and_then(|scope| self.scopes.outer(scope));
    }

    fn declare(&mut self, idents: &[IdentId], scope: Option<ScopeId>, kind: ObjectKind) {
        for ident in idents {
            let object = self.scopes.new_object(kind, false);
            self.bind(*ident, Binding::Bound(object));
            if let Some(scope) = scope {
                self.scopes.insert(scope, self.file.name(*ident), object);
            }
        }
    }

    fn declared_receiver_type_param(&mut self, index: &Expr) -> bool {
        let Expr::Ident(ident) = index else {
            return false;
        };
        let object = self.scopes.new_object(ObjectKind::Type, true);
        if let Some(scope) = self.top {
            self.scopes.insert(scope, self.file.name(*ident), object);
        }
        true
    }

    fn short_var_decl(&mut self, left: &[Expr]) {
        for expr in left {
            let Expr::Ident(ident) = expr else {
                continue;
            };
            let object = self.scopes.new_object(ObjectKind::Var, false);
            self.bind(*ident, Binding::Bound(object));
            let existing = self
                .top
                .and_then(|scope| self.scopes.insert(scope, self.file.name(*ident), object));
            if let Some(existing) = existing {
                self.bind(*ident, Binding::Bound(existing));
            }
        }
    }

    fn resolve(&mut self, ident: IdentId, collect_unresolved: bool) {
        let name = self.file.name(ident);
        if name == BLANK {
            return;
        }
        match self.scopes.lookup_chain(self.top, name) {
            Some(object) => {
                if !self.scopes.is_hidden(object) {
                    self.bind(ident, Binding::Bound(object));
                }
            }
            None if collect_unresolved => {
                self.bind(ident, Binding::Unresolved);
                self.unresolved.push(ident);
            }
            None => {}
        }
    }

    fn walk_exprs(&mut self, exprs: &[Expr]) {
        for expr in exprs {
            walk_expr(self, expr);
        }
    }

    fn walk_stmts(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            walk_stmt(self, stmt);
        }
    }

    fn walk_block(&mut self, stmts: &[Stmt]) {
        self.open_scope();
        self.walk_stmts(stmts);
        self.close_scope();
    }

    fn walk_body(&mut self, body: &[Stmt]) {
        self.labels.open();
        self.walk_stmts(body);
        let file = self.file;
        for (target, object) in self.labels.close(|ident| file.name(ident)) {
            self.bind(target, object.map_or(Binding::Free, Binding::Bound));
        }
    }

    fn resolve_list(&mut self, fields: Option<&[Field]>) {
        for field in fields.unwrap_or_default() {
            if let Some(typ) = &field.typ {
                walk_expr(self, typ);
            }
        }
    }

    fn declare_list(&mut self, fields: Option<&[Field]>, kind: ObjectKind) {
        for field in fields.unwrap_or_default() {
            self.declare(&field.names, self.top, kind);
        }
    }

    fn walk_field_list(&mut self, fields: &[Field], kind: ObjectKind) {
        self.resolve_list(Some(fields));
        self.declare_list(Some(fields), kind);
    }

    fn walk_type_params(&mut self, fields: &[Field]) {
        self.declare_list(Some(fields), ObjectKind::Type);
        self.resolve_list(Some(fields));
    }

    fn walk_func_type(&mut self, typ: &FuncType) {
        self.resolve_list(Some(&typ.params));
        self.resolve_list(typ.results.as_deref());
        self.declare_list(Some(&typ.params), ObjectKind::Var);
        self.declare_list(typ.results.as_deref(), ObjectKind::Var);
    }

    fn func_decl(&mut self, function: &FuncDecl) {
        self.open_scope();
        self.walk_receiver(function.receiver.as_deref().unwrap_or_default());
        if let Some(type_params) = &function.typ.type_params {
            self.walk_type_params(type_params);
        }
        self.resolve_list(Some(&function.typ.params));
        self.resolve_list(function.typ.results.as_deref());
        self.declare_list(function.receiver.as_deref(), ObjectKind::Var);
        self.declare_list(Some(&function.typ.params), ObjectKind::Var);
        self.declare_list(function.typ.results.as_deref(), ObjectKind::Var);
        if let Some(body) = &function.body {
            self.walk_body(body);
        }
        if function.receiver.is_none() && self.file.name(function.name) != INIT_FUNCTION {
            self.declare(&[function.name], Some(PACKAGE_SCOPE), ObjectKind::Func);
        }
        self.close_scope();
    }

    fn walk_receiver(&mut self, receiver: &[Field]) {
        let Some((first, rest)) = receiver.split_first() else {
            return;
        };
        let mut receiver_type = first.typ.as_ref();
        if let Some(Expr::Star(inner)) = receiver_type {
            receiver_type = Some(inner);
        }
        let mut resolved: Vec<&Expr> = Vec::new();
        if let Some(Expr::Index { operand, indices }) = receiver_type {
            resolved.push(operand);
            resolved.extend(
                indices
                    .iter()
                    .filter(|index| !self.declared_receiver_type_param(index)),
            );
        } else {
            resolved.extend(receiver_type);
        }
        for expr in resolved {
            walk_expr(self, expr);
        }
        for field in rest {
            if let Some(typ) = &field.typ {
                walk_expr(self, typ);
            }
        }
    }
}
