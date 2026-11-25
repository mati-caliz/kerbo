use crate::formatter::python::ast::{Expr, NodeId};
use crate::formatter::python::context::Formatter;

pub fn left_most(f: &Formatter<'_>, expression: NodeId) -> NodeId {
    let mut current = expression;
    while let Some(left) = left_child(f, current) {
        if f.is_expression_parenthesized(left) {
            break;
        }
        current = left;
    }
    current
}

fn left_child(f: &Formatter<'_>, expression: NodeId) -> Option<NodeId> {
    match f.ast.expr(expression)? {
        Expr::BinOp { left, .. }
        | Expr::Compare { left, .. }
        | Expr::IfExp { body: left, .. }
        | Expr::Call { func: left, .. }
        | Expr::Attribute { value: left, .. }
        | Expr::Subscript { value: left, .. } => Some(*left),
        Expr::BoolOp { values, .. } => values.first().copied(),
        Expr::Generator {
            elt,
            parenthesized: false,
            ..
        } => Some(*elt),
        Expr::Tuple {
            elts,
            parenthesized: false,
        } => elts.first().copied(),
        Expr::Slice { lower, .. } => *lower,
        Expr::Named { .. }
        | Expr::UnaryOp { .. }
        | Expr::Lambda { .. }
        | Expr::Dict { .. }
        | Expr::Set { .. }
        | Expr::ListComp { .. }
        | Expr::SetComp { .. }
        | Expr::DictComp { .. }
        | Expr::Generator { .. }
        | Expr::Await { .. }
        | Expr::Yield { .. }
        | Expr::YieldFrom { .. }
        | Expr::StringLiteral { .. }
        | Expr::BytesLiteral { .. }
        | Expr::NumberLiteral(_)
        | Expr::BooleanLiteral(_)
        | Expr::NoneLiteral
        | Expr::EllipsisLiteral
        | Expr::Starred { .. }
        | Expr::Name
        | Expr::List { .. }
        | Expr::Tuple { .. } => None,
    }
}
