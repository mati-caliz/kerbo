use super::call_chain::CallChainLayout;
use super::own_parentheses::has_parentheses;
use super::simple::unary_needs_line_break;
use crate::formatter::python::ast::{Expr, NodeId, Other, Stmt};
use crate::formatter::python::context::Formatter;
use crate::formatter::python::parentheses::OptionalParentheses;
use crate::formatter::python::string;

fn is_await(f: &Formatter<'_>, node: NodeId) -> bool {
    matches!(f.ast.expr(node), Some(Expr::Await { .. }))
}

fn always_inside_await(
    f: &Formatter<'_>,
    parent: NodeId,
    otherwise: OptionalParentheses,
) -> OptionalParentheses {
    if is_await(f, parent) {
        OptionalParentheses::Always
    } else {
        otherwise
    }
}

// Si `expression` es la anotación de `parent`: la de una asignación o el tipo de retorno de un def.
pub fn is_type_annotation_of(f: &Formatter<'_>, expression: NodeId, parent: NodeId) -> bool {
    match f.ast.stmt(parent) {
        Some(Stmt::AnnAssign { annotation, .. }) => *annotation == expression,
        Some(Stmt::FunctionDef { returns, .. }) => *returns == Some(expression),
        Some(_) | None => false,
    }
}

fn is_assignment(f: &Formatter<'_>, node: NodeId) -> bool {
    matches!(
        f.ast.stmt(node),
        Some(Stmt::Assign { .. } | Stmt::AnnAssign { .. } | Stmt::AugAssign { .. })
    )
}

fn named_needs_parentheses(f: &Formatter<'_>, parent: NodeId) -> OptionalParentheses {
    let in_statement = is_assignment(f, parent)
        || matches!(
            f.ast.stmt(parent),
            Some(
                Stmt::Assert { .. }
                    | Stmt::Return { .. }
                    | Stmt::Delete { .. }
                    | Stmt::For { .. }
                    | Stmt::FunctionDef { .. }
            )
        );
    let in_expression = matches!(
        f.ast.expr(parent),
        Some(Expr::Yield { .. } | Expr::YieldFrom { .. } | Expr::Await { .. } | Expr::Lambda { .. })
    );
    let in_other = matches!(
        f.ast.other(parent),
        Some(Other::ExceptHandler { .. } | Other::WithItem { .. })
    );
    if in_statement || in_expression || in_other {
        OptionalParentheses::Always
    } else {
        OptionalParentheses::Multiline
    }
}

fn string_needs_parentheses(f: &Formatter<'_>, expression: NodeId) -> OptionalParentheses {
    if string::is_implicit_concatenated(f, expression) {
        OptionalParentheses::Multiline
    } else if string::is_multiline(f, expression) || string::has_multiline_replacement_field(f, expression) {
        OptionalParentheses::Never
    } else {
        OptionalParentheses::BestFit
    }
}

fn is_string_like(f: &Formatter<'_>, expression: NodeId) -> bool {
    matches!(
        f.ast.expr(expression),
        Some(Expr::StringLiteral { .. } | Expr::BytesLiteral { .. })
    )
}

// `"""...""" % (a, b)`: una cadena de varias líneas seguida de algo con paréntesis propios no
// gana nada con paréntesis alrededor.
fn multiline_string_operand(f: &Formatter<'_>, string: NodeId, other: NodeId) -> OptionalParentheses {
    let comments = f.comments;
    let hugs = is_string_like(f, string)
        && !string::is_implicit_concatenated(f, string)
        && string::is_multiline(f, string)
        && has_parentheses(f, other).is_some()
        && !comments.has(string)
        && !comments.has(other);
    if hugs {
        OptionalParentheses::Never
    } else {
        OptionalParentheses::Multiline
    }
}

fn delegated(f: &Formatter<'_>, expression: NodeId, inner: NodeId) -> OptionalParentheses {
    if f.is_expression_parenthesized(inner) {
        OptionalParentheses::Never
    } else {
        needs_parentheses(f, inner, expression)
    }
}

fn attribute_needs_parentheses(f: &Formatter<'_>, expression: NodeId, value: NodeId) -> OptionalParentheses {
    if CallChainLayout::from_expression(f, expression).is_fluent() {
        return OptionalParentheses::Multiline;
    }
    if f.comments.has_dangling(expression) {
        return OptionalParentheses::Always;
    }
    if !f.is_expression_parenthesized(value) {
        return needs_parentheses(f, value, expression);
    }
    let has_end_of_line_comment = f
        .comments
        .trailing(value)
        .iter()
        .any(|comment| !comment.is_own_line());
    if has_end_of_line_comment {
        OptionalParentheses::Multiline
    } else {
        OptionalParentheses::Never
    }
}

fn parameters_are_empty(f: &Formatter<'_>, parameters: NodeId) -> bool {
    f.ast.children(parameters).is_empty()
}

fn subscript_needs_parentheses(
    f: &Formatter<'_>,
    expression: NodeId,
    value: NodeId,
    parent: NodeId,
) -> OptionalParentheses {
    if CallChainLayout::from_expression(f, expression).is_fluent() {
        return OptionalParentheses::Multiline;
    }
    let needs = delegated(f, expression, value);
    if needs != OptionalParentheses::BestFit {
        return needs;
    }
    match f.ast.stmt(parent) {
        Some(Stmt::FunctionDef {
            returns, parameters, ..
        }) if *returns == Some(expression) => {
            if parameters_are_empty(f, *parameters) && !f.comments.has(*parameters) {
                OptionalParentheses::Multiline
            } else {
                OptionalParentheses::Never
            }
        }
        Some(_) | None => OptionalParentheses::BestFit,
    }
}

fn yield_needs_parentheses(
    f: &Formatter<'_>,
    expression: NodeId,
    value: Option<NodeId>,
    parent: NodeId,
) -> OptionalParentheses {
    if is_type_annotation_of(f, expression, parent) || !is_assignment(f, parent) {
        return OptionalParentheses::Always;
    }
    let Some(value) = value else {
        return OptionalParentheses::Never;
    };
    match delegated(f, expression, value) {
        OptionalParentheses::BestFit => OptionalParentheses::Never,
        needs @ (OptionalParentheses::Multiline
        | OptionalParentheses::Always
        | OptionalParentheses::Never) => needs,
    }
}

fn unary_needs_parentheses(
    f: &Formatter<'_>,
    expression: NodeId,
    operand: NodeId,
    parent: NodeId,
) -> OptionalParentheses {
    if is_await(f, parent) || unary_needs_line_break(f, operand) {
        return OptionalParentheses::Always;
    }
    if f.is_expression_parenthesized(operand) {
        return OptionalParentheses::Never;
    }
    if f.comments.has(operand) {
        return OptionalParentheses::Always;
    }
    needs_parentheses(f, operand, expression)
}

fn call_needs_parentheses(f: &Formatter<'_>, expression: NodeId, func: NodeId) -> OptionalParentheses {
    if CallChainLayout::from_expression(f, expression).is_fluent() {
        OptionalParentheses::Multiline
    } else if f.comments.has_dangling(expression) {
        OptionalParentheses::Always
    } else {
        delegated(f, expression, func)
    }
}

fn operator_needs_parentheses(
    f: &Formatter<'_>,
    expression: NodeId,
    data: &Expr,
    parent: NodeId,
) -> OptionalParentheses {
    if is_await(f, parent) {
        return OptionalParentheses::Always;
    }
    if let Expr::BinOp { left, right, .. } = data
        && !f.comments.has_dangling(expression)
    {
        return multiline_string_operand(f, *left, *right);
    }
    if let Expr::Compare {
        left, comparators, ..
    } = data
        && let Some(second) = comparators.first()
    {
        return multiline_string_operand(f, *left, *second);
    }
    OptionalParentheses::Multiline
}

// Lo que la expresión pide de los paréntesis opcionales que la rodean, según su padre.
pub fn needs_parentheses(f: &Formatter<'_>, expression: NodeId, parent: NodeId) -> OptionalParentheses {
    let Some(data) = f.ast.expr(expression) else {
        return OptionalParentheses::Never;
    };
    match data {
        Expr::Name
        | Expr::NumberLiteral(_)
        | Expr::BooleanLiteral(_)
        | Expr::NoneLiteral
        | Expr::EllipsisLiteral => OptionalParentheses::BestFit,
        Expr::StringLiteral { .. } | Expr::BytesLiteral { .. } => string_needs_parentheses(f, expression),
        Expr::Starred { .. } | Expr::Slice { .. } => OptionalParentheses::Multiline,
        Expr::BoolOp { .. } | Expr::BinOp { .. } | Expr::Compare { .. } => {
            operator_needs_parentheses(f, expression, data, parent)
        }
        Expr::Lambda { .. } | Expr::IfExp { .. } => {
            always_inside_await(f, parent, OptionalParentheses::Multiline)
        }
        Expr::Generator { .. } => always_inside_await(f, parent, OptionalParentheses::Never),
        Expr::Dict { .. }
        | Expr::Set { .. }
        | Expr::List { .. }
        | Expr::Tuple { .. }
        | Expr::ListComp { .. }
        | Expr::SetComp { .. }
        | Expr::DictComp { .. } => OptionalParentheses::Never,
        Expr::Named { .. } => named_needs_parentheses(f, parent),
        Expr::Attribute { value, .. } => attribute_needs_parentheses(f, expression, *value),
        Expr::Await { value } => {
            if is_await(f, parent) || is_type_annotation_of(f, expression, parent) {
                OptionalParentheses::Always
            } else {
                delegated(f, expression, *value)
            }
        }
        Expr::UnaryOp { operand, .. } => unary_needs_parentheses(f, expression, *operand, parent),
        Expr::Yield { value } => yield_needs_parentheses(f, expression, *value, parent),
        Expr::YieldFrom { value } => yield_needs_parentheses(f, expression, Some(*value), parent),
        Expr::Call { func, .. } => call_needs_parentheses(f, expression, *func),
        Expr::Subscript { value, .. } => subscript_needs_parentheses(f, expression, *value, parent),
    }
}
