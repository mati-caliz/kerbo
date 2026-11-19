use std::cmp::Ordering;

use super::own_parentheses::{OwnParentheses, has_parentheses};
use crate::formatter::python::ast::{Expr, NodeId, Operator, UnaryOperator};
use crate::formatter::python::context::Formatter;

const CONDITIONAL_OPERATORS: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OperatorPrecedence {
    None,
    Attribute,
    Exponential,
    BitwiseInversion,
    Multiplicative,
    Additive,
    Shift,
    BitwiseAnd,
    BitwiseXor,
    BitwiseOr,
    Comparator,
    BooleanOperation,
    Conditional,
}

impl From<Operator> for OperatorPrecedence {
    fn from(operator: Operator) -> Self {
        match operator {
            Operator::Add | Operator::Sub => Self::Additive,
            Operator::Mult | Operator::MatMult | Operator::Div | Operator::Mod | Operator::FloorDiv => {
                Self::Multiplicative
            }
            Operator::Pow => Self::Exponential,
            Operator::LShift | Operator::RShift => Self::Shift,
            Operator::BitOr => Self::BitwiseOr,
            Operator::BitXor => Self::BitwiseXor,
            Operator::BitAnd => Self::BitwiseAnd,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum First {
    None,
    Token,
    Expression(NodeId),
}

fn count(length: usize) -> u32 {
    u32::try_from(length).unwrap_or(u32::MAX)
}

fn operator_precedence(data: &Expr) -> Option<(OperatorPrecedence, u32)> {
    if let Expr::BoolOp { values, .. } = data {
        return Some((
            OperatorPrecedence::BooleanOperation,
            count(values.len().saturating_sub(1)),
        ));
    }
    if let Expr::BinOp { op, .. } = data {
        return Some((OperatorPrecedence::from(*op), 1));
    }
    if let Expr::Compare { ops, .. } = data {
        return Some((OperatorPrecedence::Comparator, count(ops.len())));
    }
    matches!(data, Expr::IfExp { .. }).then_some((OperatorPrecedence::Conditional, CONDITIONAL_OPERATORS))
}

// Recorre la expresión sin entrar en paréntesis y anota el operador de menor prioridad, cuántas
// veces aparece, y con qué empieza y termina la expresión.
#[derive(Debug)]
struct Visitor<'formatter, 'source> {
    f: &'formatter Formatter<'source>,
    max_precedence: OperatorPrecedence,
    max_precedence_count: u32,
    any_parenthesized_expressions: bool,
    last: Option<NodeId>,
    first: First,
}

impl Visitor<'_, '_> {
    fn update_max_precedence(&mut self, precedence: OperatorPrecedence, count: u32) {
        match self.max_precedence.cmp(&precedence) {
            Ordering::Less => {
                self.max_precedence_count = count;
                self.max_precedence = precedence;
            }
            Ordering::Equal => self.max_precedence_count = self.max_precedence_count.saturating_add(count),
            Ordering::Greater => {}
        }
    }

    const fn set_first_if_none(&mut self, first: First) {
        if matches!(self.first, First::None) {
            self.first = first;
        }
    }

    fn visit_expr(&mut self, expression: NodeId) {
        self.last = Some(expression);
        if self.f.is_expression_parenthesized(expression) {
            self.any_parenthesized_expressions = true;
        } else {
            self.visit_subexpression(expression);
        }
        self.set_first_if_none(First::Expression(expression));
    }

    fn walk(&mut self, node: NodeId) {
        for child in self.f.ast.children(node) {
            if self.f.ast.data(child).is_expression() {
                self.visit_expr(child);
            } else {
                self.walk(child);
            }
        }
    }

    // Lo que se visita sin recorrer los hijos devuelve `false`.
    fn visit_operator(&mut self, expression: NodeId, data: &Expr) -> bool {
        if let Some((precedence, count)) = operator_precedence(data) {
            self.update_max_precedence(precedence, count);
            return true;
        }
        if let Expr::Call { func: inner, .. } | Expr::Subscript { value: inner, .. } = data {
            self.any_parenthesized_expressions = true;
            self.visit_expr(*inner);
            self.last = Some(expression);
            return false;
        }
        if let Expr::Attribute { value, .. } = data {
            self.visit_expr(*value);
            if has_parentheses(self.f, *value).is_some() {
                self.update_max_precedence(OperatorPrecedence::Attribute, 1);
            }
            self.last = Some(expression);
            return false;
        }
        if let Expr::UnaryOp {
            op: UnaryOperator::Invert,
            ..
        } = data
        {
            self.update_max_precedence(OperatorPrecedence::BitwiseInversion, 1);
        }
        self.set_first_if_none(First::Token);
        true
    }

    fn visit_subexpression(&mut self, expression: NodeId) {
        let Some(data) = self.f.ast.expr(expression) else {
            return;
        };
        let walks_children = match data {
            Expr::Dict { .. }
            | Expr::List { .. }
            | Expr::Set { .. }
            | Expr::ListComp { .. }
            | Expr::SetComp { .. }
            | Expr::DictComp { .. }
            | Expr::Tuple {
                parenthesized: true, ..
            }
            | Expr::Generator {
                parenthesized: true, ..
            } => {
                self.any_parenthesized_expressions = true;
                false
            }
            Expr::Named { .. } | Expr::Generator { .. } | Expr::Tuple { .. } => true,
            Expr::StringLiteral { .. }
            | Expr::BytesLiteral { .. }
            | Expr::NumberLiteral(_)
            | Expr::BooleanLiteral(_)
            | Expr::NoneLiteral
            | Expr::EllipsisLiteral
            | Expr::Name
            | Expr::Slice { .. } => false,
            Expr::BoolOp { .. }
            | Expr::BinOp { .. }
            | Expr::IfExp { .. }
            | Expr::Compare { .. }
            | Expr::Call { .. }
            | Expr::Subscript { .. }
            | Expr::Attribute { .. }
            | Expr::UnaryOp { .. }
            | Expr::Lambda { .. }
            | Expr::Await { .. }
            | Expr::Yield { .. }
            | Expr::YieldFrom { .. }
            | Expr::Starred { .. } => self.visit_operator(expression, data),
        };
        if walks_children {
            self.walk(expression);
        }
    }
}

fn is_parenthesized(f: &Formatter<'_>, expression: NodeId) -> bool {
    !matches!(f.ast.expr(expression), Some(Expr::Subscript { .. }))
        && has_parentheses(f, expression) == Some(OwnParentheses::NonEmpty)
}

// Si los paréntesis opcionales se pueden omitir porque la expresión ya tiene dónde partirse: una
// llamada, un subíndice o una colección al principio o al final.
pub fn can_omit_optional_parentheses(f: &Formatter<'_>, expression: NodeId) -> bool {
    let mut visitor = Visitor {
        f,
        max_precedence: OperatorPrecedence::None,
        max_precedence_count: 0,
        any_parenthesized_expressions: false,
        last: None,
        first: First::None,
    };
    visitor.visit_subexpression(expression);
    if !visitor.any_parenthesized_expressions || visitor.max_precedence_count > 1 {
        return false;
    }
    if matches!(
        visitor.max_precedence,
        OperatorPrecedence::None | OperatorPrecedence::Attribute
    ) {
        return true;
    }
    let first = match visitor.first {
        First::Expression(first) => Some(first),
        First::None | First::Token => None,
    };
    visitor.last.is_some_and(|last| is_parenthesized(f, last))
        || first.is_some_and(|first| is_parenthesized(f, first))
}
