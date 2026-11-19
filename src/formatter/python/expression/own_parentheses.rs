use crate::formatter::python::ast::{Expr, NodeId, Other};
use crate::formatter::python::context::Formatter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnParentheses {
    Empty,
    NonEmpty,
}

fn non_empty_if(f: &Formatter<'_>, expression: NodeId, has_content: bool) -> OwnParentheses {
    if has_content || f.comments.has_dangling(expression) {
        OwnParentheses::NonEmpty
    } else {
        OwnParentheses::Empty
    }
}

pub fn arguments_are_empty(f: &Formatter<'_>, arguments: NodeId) -> bool {
    match f.ast.other(arguments) {
        Some(Other::Arguments { args, keywords }) => args.is_empty() && keywords.is_empty(),
        Some(_) | None => true,
    }
}

// Si la expresión trae sus propios paréntesis, corchetes o llaves (los de una llamada cuentan), y
// si tienen algo adentro.
pub fn has_own_parentheses(f: &Formatter<'_>, expression: NodeId) -> Option<OwnParentheses> {
    match f.ast.expr(expression)? {
        Expr::ListComp { .. } | Expr::SetComp { .. } | Expr::DictComp { .. } | Expr::Subscript { .. } => {
            Some(OwnParentheses::NonEmpty)
        }
        Expr::Generator { parenthesized, .. } => parenthesized.then_some(OwnParentheses::NonEmpty),
        Expr::List { elts } | Expr::Set { elts } => Some(non_empty_if(f, expression, !elts.is_empty())),
        Expr::Tuple { elts, parenthesized } => {
            parenthesized.then(|| non_empty_if(f, expression, !elts.is_empty()))
        }
        Expr::Dict { items } => Some(non_empty_if(f, expression, !items.is_empty())),
        Expr::Call { arguments, .. } => {
            Some(non_empty_if(f, expression, !arguments_are_empty(f, *arguments)))
        }
        Expr::BoolOp { .. }
        | Expr::Named { .. }
        | Expr::BinOp { .. }
        | Expr::UnaryOp { .. }
        | Expr::Lambda { .. }
        | Expr::IfExp { .. }
        | Expr::Await { .. }
        | Expr::Yield { .. }
        | Expr::YieldFrom { .. }
        | Expr::Compare { .. }
        | Expr::StringLiteral { .. }
        | Expr::BytesLiteral { .. }
        | Expr::NumberLiteral(_)
        | Expr::BooleanLiteral(_)
        | Expr::NoneLiteral
        | Expr::EllipsisLiteral
        | Expr::Attribute { .. }
        | Expr::Starred { .. }
        | Expr::Name
        | Expr::Slice { .. } => None,
    }
}

pub fn has_parentheses(f: &Formatter<'_>, expression: NodeId) -> Option<OwnParentheses> {
    let own_parentheses = has_own_parentheses(f, expression);
    if own_parentheses == Some(OwnParentheses::NonEmpty) {
        return own_parentheses;
    }
    if f.is_expression_parenthesized(expression) {
        return Some(OwnParentheses::NonEmpty);
    }
    own_parentheses
}

const fn is_implicit_concatenated(parts: &[NodeId]) -> bool {
    parts.len() > 1
}

// Si la expresión se puede partir en más de una línea.
pub fn is_splittable_expression(f: &Formatter<'_>, expression: NodeId) -> bool {
    let Some(data) = f.ast.expr(expression) else {
        return false;
    };
    match data {
        Expr::Named { .. }
        | Expr::Name
        | Expr::NumberLiteral(_)
        | Expr::BooleanLiteral(_)
        | Expr::NoneLiteral
        | Expr::EllipsisLiteral
        | Expr::Slice { .. } => false,
        Expr::Compare { .. }
        | Expr::BinOp { .. }
        | Expr::BoolOp { .. }
        | Expr::IfExp { .. }
        | Expr::Generator { .. }
        | Expr::Subscript { .. }
        | Expr::Await { .. }
        | Expr::ListComp { .. }
        | Expr::SetComp { .. }
        | Expr::DictComp { .. }
        | Expr::YieldFrom { .. } => true,
        Expr::Tuple { elts, .. } | Expr::Set { elts } | Expr::List { elts } => !elts.is_empty(),
        Expr::Dict { items } => !items.is_empty(),
        Expr::UnaryOp { operand, .. } => is_splittable_expression(f, *operand),
        Expr::Yield { value } => value.is_some(),
        Expr::Call { arguments, func } => {
            !arguments_are_empty(f, *arguments) || f.is_expression_parenthesized(*func)
        }
        Expr::StringLiteral { parts } | Expr::BytesLiteral { parts } => is_implicit_concatenated(parts),
        Expr::Lambda { body: inner, .. }
        | Expr::Starred { value: inner }
        | Expr::Attribute { value: inner, .. } => {
            f.is_expression_parenthesized(*inner) || is_splittable_expression(f, *inner)
        }
    }
}
