use crate::formatter::python::ast::{BoolOperator, CmpOperator, Expr, NodeId, Operator};
use crate::formatter::python::comments::SourceComment;
use crate::formatter::python::context::Formatter;
use crate::formatter::python::expression::can_omit::OperatorPrecedence;

#[derive(Debug, Clone, Copy)]
pub enum Symbol {
    Binary(Operator),
    Comparator(CmpOperator),
    Bool(BoolOperator),
}

impl Symbol {
    pub const fn is_pow(self) -> bool {
        matches!(self, Self::Binary(Operator::Pow))
    }

    pub fn precedence(self) -> OperatorPrecedence {
        match self {
            Self::Binary(operator) => OperatorPrecedence::from(operator),
            Self::Comparator(_) => OperatorPrecedence::Comparator,
            Self::Bool(_) => OperatorPrecedence::BooleanOperation,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Binary(operator) => operator.as_str(),
            Self::Comparator(operator) => operator.as_str(),
            Self::Bool(operator) => operator.as_str(),
        }
    }
}

// El operando de más a la izquierda de una expresión anidada lleva los comentarios que estaban
// antes de ella, y el de más a la derecha los de después: así salen donde estaban en el fuente.
#[derive(Debug, Clone, Copy)]
pub struct Operand<'comments> {
    pub expression: NodeId,
    pub leading: Option<&'comments [SourceComment]>,
    pub trailing: Option<&'comments [SourceComment]>,
}

#[derive(Debug, Clone, Copy)]
pub struct OperatorPart<'comments> {
    pub symbol: Symbol,
    pub trailing_comments: &'comments [SourceComment],
}

impl OperatorPart<'_> {
    pub const fn has_trailing_comments(&self) -> bool {
        !self.trailing_comments.is_empty()
    }
}

// Operandos en las posiciones pares y operadores en las impares.
#[derive(Debug, Clone, Copy)]
pub enum Part<'comments> {
    Operand(Operand<'comments>),
    Operator(OperatorPart<'comments>),
}

type Edges<'comments> = (
    Option<&'comments [SourceComment]>,
    Option<&'comments [SourceComment]>,
);

// Los operandos de `expression` en orden, cada uno con el símbolo que lo precede.
fn operands_of(f: &Formatter<'_>, expression: NodeId) -> Option<Vec<(Option<Symbol>, NodeId)>> {
    let data = f.ast.expr(expression)?;
    if let Expr::BinOp { left, op, right } = data {
        return Some(vec![(None, *left), (Some(Symbol::Binary(*op)), *right)]);
    }
    if let Expr::Compare {
        left,
        ops,
        comparators,
    } = data
    {
        let rest = ops
            .iter()
            .zip(comparators)
            .map(|(op, comparator)| (Some(Symbol::Comparator(*op)), *comparator));
        return Some(std::iter::once((None, *left)).chain(rest).collect());
    }
    if let Expr::BoolOp { op, values } = data {
        let operands = values
            .iter()
            .enumerate()
            .map(|(index, value)| ((index > 0).then_some(Symbol::Bool(*op)), *value));
        return Some(operands.collect());
    }
    None
}

fn expand<'comments>(
    f: &Formatter<'comments>,
    expression: NodeId,
    operands: Vec<(Option<Symbol>, NodeId)>,
    parts: &mut Vec<Part<'comments>>,
) -> impl FnMut(Edges<'comments>) {
    let comments = f.comments;
    let is_binary = matches!(f.ast.expr(expression), Some(Expr::BinOp { .. }));
    let operator_comments: &[SourceComment] = if is_binary {
        comments.dangling(expression)
    } else {
        &[]
    };
    move |(leading, trailing)| {
        let last = operands.len().saturating_sub(1);
        for (index, (symbol, operand)) in operands.iter().enumerate() {
            if let Some(symbol) = symbol {
                parts.push(Part::Operator(OperatorPart {
                    symbol: *symbol,
                    trailing_comments: operator_comments,
                }));
            }
            let edges = (
                if index == 0 { leading } else { None },
                if index == last && index > 0 {
                    trailing
                } else {
                    None
                },
            );
            recurse(f, *operand, edges, parts);
        }
    }
}

fn recurse<'comments>(
    f: &Formatter<'comments>,
    expression: NodeId,
    (leading, trailing): Edges<'comments>,
    parts: &mut Vec<Part<'comments>>,
) {
    let nested = if f.is_expression_parenthesized(expression) {
        None
    } else {
        operands_of(f, expression)
    };
    let Some(operands) = nested else {
        parts.push(Part::Operand(Operand {
            expression,
            leading,
            trailing,
        }));
        return;
    };
    let comments = f.comments;
    let edges = (
        Some(leading.unwrap_or_else(|| comments.leading(expression))),
        Some(trailing.unwrap_or_else(|| comments.trailing(expression))),
    );
    expand(f, expression, operands, parts)(edges);
}

pub fn flatten<'comments>(f: &Formatter<'comments>, expression: NodeId) -> Vec<Part<'comments>> {
    let mut parts = Vec::new();
    if let Some(operands) = operands_of(f, expression) {
        let empty: &[SourceComment] = &[];
        expand(f, expression, operands, &mut parts)((Some(empty), Some(empty)));
    }
    parts
}

pub fn between_operators<'parts, 'comments>(
    parts: &'parts [Part<'comments>],
    last_operator: Option<usize>,
    end: usize,
) -> &'parts [Part<'comments>] {
    let start = last_operator.map_or(0, |operator| operator.saturating_add(1));
    parts.get(start..end).unwrap_or_default()
}

pub fn after_operator<'parts, 'comments>(
    parts: &'parts [Part<'comments>],
    operator: usize,
) -> &'parts [Part<'comments>] {
    parts.get(operator.saturating_add(1)..).unwrap_or_default()
}

pub fn lowest_precedence(parts: &[Part<'_>]) -> OperatorPrecedence {
    parts
        .iter()
        .filter_map(|part| match part {
            Part::Operator(operator) => Some(operator.symbol.precedence()),
            Part::Operand(_) => None,
        })
        .max()
        .unwrap_or(OperatorPrecedence::None)
}

const fn operand_of<'parts, 'comments>(
    part: Option<&'parts Part<'comments>>,
) -> Option<&'parts Operand<'comments>> {
    match part {
        Some(Part::Operand(operand)) => Some(operand),
        Some(Part::Operator(_)) | None => None,
    }
}

pub const fn first_operand<'parts, 'comments>(
    parts: &'parts [Part<'comments>],
) -> Option<&'parts Operand<'comments>> {
    operand_of(parts.first())
}

pub const fn last_operand<'parts, 'comments>(
    parts: &'parts [Part<'comments>],
) -> Option<&'parts Operand<'comments>> {
    operand_of(parts.last())
}
