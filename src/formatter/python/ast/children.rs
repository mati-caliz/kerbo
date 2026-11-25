use super::data::{Expr, Other, Stmt};
use super::{Data, NodeId};

fn joined(groups: &[&[NodeId]]) -> Vec<NodeId> {
    groups.iter().flat_map(|group| group.iter().copied()).collect()
}

fn optional(node: Option<NodeId>) -> Vec<NodeId> {
    node.into_iter().collect()
}

fn of_definition(statement: &Stmt) -> Vec<NodeId> {
    if let Stmt::FunctionDef {
        decorators,
        type_params,
        parameters,
        returns,
        body,
        ..
    } = statement
    {
        return joined(&[
            decorators,
            &optional(*type_params),
            &[*parameters],
            &optional(*returns),
            body,
        ]);
    }
    if let Stmt::ClassDef {
        decorators,
        type_params,
        arguments,
        body,
        ..
    } = statement
    {
        return joined(&[decorators, &optional(*type_params), &optional(*arguments), body]);
    }
    if let Stmt::Try {
        body,
        handlers,
        orelse,
        finalbody,
        ..
    } = statement
    {
        return joined(&[body, handlers, orelse, finalbody]);
    }
    if let Stmt::For {
        target,
        iter,
        body,
        orelse,
        ..
    } = statement
    {
        return joined(&[&[*target, *iter], body, orelse]);
    }
    Vec::new()
}

fn of_statement(statement: &Stmt) -> Vec<NodeId> {
    match statement {
        Stmt::Return { value } => optional(*value),
        Stmt::Delete { targets: nodes }
        | Stmt::Import { names: nodes, .. }
        | Stmt::ImportFrom { names: nodes, .. } => nodes.clone(),
        Stmt::Assign { targets, value } => joined(&[targets, &[*value]]),
        Stmt::AugAssign { target, value, .. } => vec![*target, *value],
        Stmt::AnnAssign {
            target,
            annotation,
            value,
        } => joined(&[&[*target, *annotation], &optional(*value)]),
        Stmt::While {
            test,
            body,
            orelse: rest,
        }
        | Stmt::If {
            test,
            body,
            clauses: rest,
        } => joined(&[&[*test], body, rest]),
        Stmt::With { items, body, .. } => joined(&[items, body]),
        Stmt::Raise {
            exc: first,
            cause: second,
        } => joined(&[&optional(*first), &optional(*second)]),
        Stmt::Assert { test, msg } => joined(&[&[*test], &optional(*msg)]),
        Stmt::Expression { value } => vec![*value],
        Stmt::FunctionDef { .. } | Stmt::ClassDef { .. } | Stmt::Try { .. } | Stmt::For { .. } => {
            of_definition(statement)
        }
        Stmt::Global { .. } | Stmt::Nonlocal { .. } | Stmt::Pass | Stmt::Break | Stmt::Continue => Vec::new(),
    }
}

fn of_expression(expression: &Expr) -> Vec<NodeId> {
    match expression {
        Expr::BoolOp { values: nodes, .. }
        | Expr::Set { elts: nodes }
        | Expr::List { elts: nodes }
        | Expr::Tuple { elts: nodes, .. }
        | Expr::StringLiteral { parts: nodes }
        | Expr::BytesLiteral { parts: nodes } => nodes.clone(),
        Expr::Named {
            target: first,
            value: second,
        }
        | Expr::BinOp {
            left: first,
            right: second,
            ..
        }
        | Expr::Subscript {
            value: first,
            slice: second,
        }
        | Expr::Call {
            func: first,
            arguments: second,
        } => vec![*first, *second],
        Expr::UnaryOp { operand: only, .. }
        | Expr::Await { value: only }
        | Expr::YieldFrom { value: only }
        | Expr::Starred { value: only }
        | Expr::Attribute { value: only, .. } => vec![*only],
        Expr::Lambda { parameters, body } => joined(&[&optional(*parameters), &[*body]]),
        Expr::IfExp { test, body, orelse } => vec![*body, *test, *orelse],
        Expr::Dict { items } => items
            .iter()
            .flat_map(|(key, value)| key.iter().copied().chain([*value]))
            .collect(),
        Expr::ListComp { elt, generators }
        | Expr::SetComp { elt, generators }
        | Expr::Generator { elt, generators, .. } => joined(&[&[*elt], generators]),
        Expr::DictComp {
            key,
            value,
            generators,
        } => joined(&[&[*key, *value], generators]),
        Expr::Yield { value } => optional(*value),
        Expr::Compare {
            left, comparators, ..
        } => joined(&[&[*left], comparators]),
        Expr::Slice { lower, upper, step } => {
            joined(&[&optional(*lower), &optional(*upper), &optional(*step)])
        }
        Expr::NumberLiteral(_)
        | Expr::BooleanLiteral(_)
        | Expr::NoneLiteral
        | Expr::EllipsisLiteral
        | Expr::Name => Vec::new(),
    }
}

fn of_other(other: &Other) -> Vec<NodeId> {
    match other {
        Other::ExceptHandler {
            type_: head, body, ..
        }
        | Other::ElifElseClause { test: head, body } => joined(&[&optional(*head), body]),
        Other::Comprehension {
            target, iter, ifs, ..
        } => joined(&[&[*target, *iter], ifs]),
        Other::Arguments { args, keywords } => joined(&[args, keywords]),
        Other::Keyword { value: only, .. } | Other::Decorator { expression: only } => vec![*only],
        Other::Parameters {
            posonlyargs,
            args,
            vararg,
            kwonlyargs,
            kwarg,
        } => joined(&[
            posonlyargs,
            args,
            &optional(*vararg),
            kwonlyargs,
            &optional(*kwarg),
        ]),
        Other::Parameter { annotation, .. } => optional(*annotation),
        Other::ParameterWithDefault {
            parameter: first,
            default: second,
        }
        | Other::WithItem {
            context_expr: first,
            optional_vars: second,
        } => joined(&[&[*first], &optional(*second)]),
        Other::Module { body: nodes }
        | Other::FStringPart { elements: nodes }
        | Other::FormatSpec { elements: nodes } => nodes.clone(),
        Other::Interpolation {
            expression,
            format_spec,
            ..
        } => joined(&[&[*expression], &optional(*format_spec)]),
        Other::Alias { .. } | Other::StringPart | Other::BytesPart | Other::InterpolatedLiteral => Vec::new(),
    }
}

pub fn of(data: &Data) -> Vec<NodeId> {
    match data {
        Data::Stmt(statement) => of_statement(statement),
        Data::Expr(expression) => of_expression(expression),
        Data::Other(other) => of_other(other),
    }
}
