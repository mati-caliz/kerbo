mod builder;
mod graph;
mod statements;

use std::collections::HashMap;

use self::builder::Builder;
use self::graph::{BranchStack, Graph};
use super::ast::{Decl, Expr, File, IdentId};
use super::resolver::Resolution;

const ZERO_LITERALS: [&str; 5] = ["0", "0.0", "0.", ".0", "\"\""];
const ZERO_IDENTIFIERS: [&str; 2] = ["false", "nil"];

pub fn ineffectual(file: &File, resolution: &Resolution) -> Vec<IdentId> {
    let mut builder = Builder {
        file,
        resolution,
        graph: Graph::default(),
        roots: Vec::new(),
        current: None,
        variables: HashMap::new(),
        results: Vec::new(),
        defers: Vec::new(),
        breaks: BranchStack::default(),
        continues: BranchStack::default(),
        gotos: BranchStack::default(),
        label_statement: None,
    };
    for decl in &file.decls {
        match decl {
            Decl::Func(function) => {
                if let Some(body) = &function.body {
                    builder.function(function.receiver.as_deref(), &function.typ, body);
                }
            }
            Decl::Gen(decl) => builder.var_decl(decl),
        }
    }
    let mut found = builder.graph.ineffectual(&builder.roots, &builder.variables);
    found.sort_by_key(|ident| file.ident(*ident).map(|ident| ident.offset));
    found
}

fn is_zero_initializer(expr: &Expr, resolution: &Resolution, file: &File) -> bool {
    let value = if let Expr::Call { function, arguments } = expr {
        let [argument] = arguments.as_slice() else {
            return false;
        };
        if !is_conversion_target(function) {
            return false;
        }
        argument
    } else {
        expr
    };
    if let Expr::BasicLit(text) = value {
        return ZERO_LITERALS.contains(&text.as_str());
    }
    if let Expr::Ident(ident) = value {
        return ZERO_IDENTIFIERS.contains(&file.name(*ident)) && resolution.object(*ident).is_none();
    }
    false
}

fn is_conversion_target(function: &Expr) -> bool {
    let mut target = function;
    if let Expr::Paren(inner) = target {
        target = inner;
    }
    if let Expr::Star(inner) = target {
        target = inner;
    }
    matches!(
        target,
        Expr::Ident(_)
            | Expr::Selector { .. }
            | Expr::ArrayType { .. }
            | Expr::StructType(_)
            | Expr::FuncType(_)
            | Expr::InterfaceType(_)
            | Expr::MapType { .. }
            | Expr::ChanType(_)
    )
}
