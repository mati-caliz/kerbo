use super::super::ast::{Block, Expr, FieldList, FuncType, GenDecl, Spec, Stmt};
use super::super::token::Token;

// Como `stripParens` de go/printer: los paréntesis de una condición se sacan salvo que protejan un
// literal compuesto que empieza con un nombre de tipo (`if x == (T{}) {`).
pub fn strip_parens(x: &Expr) -> &Expr {
    if let Expr::Paren(paren) = x
        && !named_literal_in(&paren.x)
    {
        return strip_parens(&paren.x);
    }
    x
}

pub fn is_composite_like(x: &Expr) -> bool {
    let inner = x.without_parens();
    if let Expr::Unary(unary) = inner {
        return unary.op == Token::And && matches!(unary.x.without_parens(), Expr::Composite(_));
    }
    matches!(inner, Expr::Composite(_))
}

fn is_type_name(x: &Expr) -> bool {
    if let Expr::Selector(selector) = x {
        return is_type_name(&selector.x);
    }
    matches!(x, Expr::Ident(_))
}

fn named_literal_in(x: &Expr) -> bool {
    match x {
        Expr::Paren(_) => false,
        Expr::Composite(literal) => literal.typ.as_ref().is_some_and(is_type_name),
        Expr::Ident(_) | Expr::BasicLit(_) => false,
        Expr::FuncLit(literal) => in_func_type(&literal.typ) || in_block(&literal.body),
        Expr::Selector(selector) => named_literal_in(&selector.x),
        Expr::Index(index) => named_literal_in(&index.x) || named_literal_in(&index.index),
        Expr::IndexList(index) => named_literal_in(&index.x) || index.indices.iter().any(named_literal_in),
        Expr::Slice(slice) => {
            named_literal_in(&slice.x)
                || [&slice.low, &slice.high, &slice.max]
                    .into_iter()
                    .flatten()
                    .any(named_literal_in)
        }
        Expr::TypeAssert(assert) => {
            named_literal_in(&assert.x) || assert.typ.as_ref().is_some_and(named_literal_in)
        }
        Expr::Call(call) => named_literal_in(&call.fun) || call.args.iter().any(named_literal_in),
        Expr::Star(star) => named_literal_in(&star.x),
        Expr::Unary(unary) => named_literal_in(&unary.x),
        Expr::Binary(binary) => named_literal_in(&binary.x) || named_literal_in(&binary.y),
        Expr::KeyValue(pair) => named_literal_in(&pair.key) || named_literal_in(&pair.value),
        Expr::Array(array) => {
            array.len.as_ref().is_some_and(named_literal_in) || named_literal_in(&array.elt)
        }
        Expr::Struct(structure) => in_fields(&structure.fields),
        Expr::Func(func) => in_func_type(func),
        Expr::Interface(interface) => in_fields(&interface.methods),
        Expr::Map(map) => named_literal_in(&map.key) || named_literal_in(&map.value),
        Expr::Chan(chan) => named_literal_in(&chan.value),
        Expr::Ellipsis(ellipsis) => ellipsis.elt.as_ref().is_some_and(named_literal_in),
    }
}

fn in_fields(fields: &FieldList) -> bool {
    fields.list.iter().any(|field| named_literal_in(&field.typ))
}

fn in_func_type(func: &FuncType) -> bool {
    func.type_params.as_ref().is_some_and(in_fields)
        || in_fields(&func.params)
        || func.results.as_ref().is_some_and(in_fields)
}

fn in_block(body: &Block) -> bool {
    body.list.iter().any(in_stmt)
}

fn in_optional(statement: Option<&Stmt>) -> bool {
    statement.is_some_and(in_stmt)
}

fn in_exprs(list: &[Expr]) -> bool {
    list.iter().any(named_literal_in)
}

fn in_stmt(statement: &Stmt) -> bool {
    match statement {
        Stmt::Decl(decl) => in_gen_decl(decl),
        Stmt::Empty(_) => false,
        Stmt::Labeled(labeled) => in_stmt(&labeled.stmt),
        Stmt::Expr(x) => named_literal_in(x),
        Stmt::Send(send) => named_literal_in(&send.chan) || named_literal_in(&send.value),
        Stmt::IncDec(statement) => named_literal_in(&statement.x),
        Stmt::Assign(assign) => in_exprs(&assign.lhs) || in_exprs(&assign.rhs),
        Stmt::Go(statement) | Stmt::Defer(statement) => named_literal_in(&statement.call),
        Stmt::Return(statement) => in_exprs(&statement.results),
        Stmt::Branch(_) => false,
        Stmt::Block(body) => in_block(body),
        Stmt::If(statement) => {
            in_optional(statement.init.as_ref())
                || named_literal_in(&statement.cond)
                || in_block(&statement.body)
                || in_optional(statement.alternative.as_ref())
        }
        Stmt::Case(clause) => clause.list.as_deref().is_some_and(in_exprs) || clause.body.iter().any(in_stmt),
        Stmt::Switch(statement) => {
            in_optional(statement.init.as_ref())
                || statement.tag.as_ref().is_some_and(named_literal_in)
                || in_block(&statement.body)
        }
        Stmt::TypeSwitch(statement) => {
            in_optional(statement.init.as_ref()) || in_stmt(&statement.assign) || in_block(&statement.body)
        }
        Stmt::Comm(clause) => in_optional(clause.comm.as_ref()) || clause.body.iter().any(in_stmt),
        Stmt::Select(statement) => in_block(&statement.body),
        Stmt::For(statement) => {
            in_optional(statement.init.as_ref())
                || statement.cond.as_ref().is_some_and(named_literal_in)
                || in_optional(statement.post.as_ref())
                || in_block(&statement.body)
        }
        Stmt::Range(statement) => {
            [&statement.key, &statement.value]
                .into_iter()
                .flatten()
                .any(named_literal_in)
                || named_literal_in(&statement.x)
                || in_block(&statement.body)
        }
    }
}

fn in_gen_decl(decl: &GenDecl) -> bool {
    decl.specs.iter().any(|spec| match spec {
        Spec::Import(_) => false,
        Spec::Value(value) => value.typ.as_ref().is_some_and(named_literal_in) || in_exprs(&value.values),
        Spec::Type(typ) => typ.type_params.as_ref().is_some_and(in_fields) || named_literal_in(&typ.typ),
    })
}
