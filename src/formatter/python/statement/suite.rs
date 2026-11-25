use super::format_statement;
use crate::formatter::document::LineMode;
use crate::formatter::python::ast::{Expr, NodeId, Other, Stmt};
use crate::formatter::python::comments::format::{leading_comments, trailing_comments};
use crate::formatter::python::context::{Formatted, Formatter, NodeLevel, TopLevelPosition};
use crate::formatter::python::string::{self, StringKind};
use crate::formatter::python::trivia::{lines_after, lines_after_ignoring_end_of_line_trivia, lines_before};

const EMPTY_LINE: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuiteKind {
    TopLevel,
    Function,
    Class,
    Other,
}

fn is_class_or_function_definition(f: &Formatter<'_>, statement: NodeId) -> bool {
    matches!(
        f.ast.stmt(statement),
        Some(Stmt::FunctionDef { .. } | Stmt::ClassDef { .. })
    )
}

fn is_import_definition(f: &Formatter<'_>, statement: NodeId) -> bool {
    matches!(
        f.ast.stmt(statement),
        Some(Stmt::Import { .. } | Stmt::ImportFrom { .. })
    )
}

fn is_compound_statement(f: &Formatter<'_>, statement: NodeId) -> bool {
    matches!(
        f.ast.stmt(statement),
        Some(
            Stmt::FunctionDef { .. }
                | Stmt::ClassDef { .. }
                | Stmt::While { .. }
                | Stmt::For { .. }
                | Stmt::If { .. }
                | Stmt::Try { .. }
                | Stmt::With { .. }
        )
    )
}

fn last_of_try(statement: &Stmt) -> Option<NodeId> {
    let Stmt::Try {
        body,
        handlers,
        orelse,
        finalbody,
        ..
    } = statement
    else {
        return None;
    };
    [finalbody, orelse, handlers, body]
        .into_iter()
        .find_map(|block| block.last().copied())
}

fn body_of_other<'ast>(f: &Formatter<'ast>, node: NodeId) -> Option<&'ast [NodeId]> {
    match f.ast.other(node)? {
        Other::ExceptHandler { body, .. } | Other::ElifElseClause { body, .. } => Some(body),
        Other::Module { .. }
        | Other::Comprehension { .. }
        | Other::Arguments { .. }
        | Other::Keyword { .. }
        | Other::Parameters { .. }
        | Other::Parameter { .. }
        | Other::ParameterWithDefault { .. }
        | Other::Alias { .. }
        | Other::WithItem { .. }
        | Other::Decorator { .. }
        | Other::StringPart
        | Other::BytesPart
        | Other::TypeParams { .. }
        | Other::TypeVar { .. }
        | Other::TypeVarTuple { .. }
        | Other::ParamSpec { .. }
        | Other::FStringPart { .. }
        | Other::InterpolatedLiteral
        | Other::Interpolation { .. }
        | Other::FormatSpec { .. } => None,
    }
}

// `AnyNodeRef::last_child_in_body` de ruff: la última sentencia del último bloque del nodo.
fn last_child_in_body(f: &Formatter<'_>, node: NodeId) -> Option<NodeId> {
    let Some(statement) = f.ast.stmt(node) else {
        return body_of_other(f, node)?.last().copied();
    };
    if matches!(statement, Stmt::Try { .. }) {
        return last_of_try(statement);
    }
    if let Stmt::If { body, clauses, .. } = statement {
        let last_body = clauses
            .last()
            .and_then(|clause| body_of_other(f, *clause))
            .unwrap_or(body);
        return last_body.last().copied();
    }
    if let Stmt::For { body, orelse, .. } | Stmt::While { body, orelse, .. } = statement {
        let last_body = if orelse.is_empty() { body } else { orelse };
        return last_body.last().copied();
    }
    if let Stmt::FunctionDef { body, .. } | Stmt::ClassDef { body, .. } | Stmt::With { body, .. } = statement
    {
        return body.last().copied();
    }
    None
}

// El último `def` o `class` con que termina la sentencia, bajando por los cuerpos anidados y sin
// pasar por un nodo que tenga un comentario propio detrás.
fn trailing_function_or_class_def(f: &Formatter<'_>, preceding: NodeId) -> Option<NodeId> {
    let mut current = Some(preceding);
    while let Some(node) = current {
        if f.comments.has_trailing_own_line(node) {
            return None;
        }
        if is_class_or_function_definition(f, node) {
            return Some(node);
        }
        current = last_child_in_body(f, node);
    }
    None
}

// El cuerpo de un stub: sólo `...`, sin comentarios que lo separen del encabezado.
pub fn as_only_an_ellipsis(f: &Formatter<'_>, body: &[NodeId]) -> Option<NodeId> {
    let [statement] = body else {
        return None;
    };
    let Some(Stmt::Expression { value }) = f.ast.stmt(*statement) else {
        return None;
    };
    let is_ellipsis = matches!(f.ast.expr(*value), Some(Expr::EllipsisLiteral));
    let has_comments = f.comments.has_leading(*statement) || f.comments.has_trailing_own_line(*statement);
    (is_ellipsis && !has_comments).then_some(*statement)
}

fn is_docstring_statement(f: &Formatter<'_>, statement: NodeId) -> bool {
    let Some(Stmt::Expression { value }) = f.ast.stmt(statement) else {
        return false;
    };
    let Some(Expr::StringLiteral { parts }) = f.ast.expr(*value) else {
        return false;
    };
    string::is_plain_string(f, *value)
        && (parts.len() == 1 || !parts.iter().any(|part| f.comments.has(*part)))
}

fn fmt_docstring(f: &mut Formatter<'_>, statement: NodeId, kind: SuiteKind) -> Formatted {
    let Some(Stmt::Expression { value }) = f.ast.stmt(statement) else {
        return format_statement(f, statement);
    };
    let comments = f.comments;
    leading_comments(f, comments.leading(statement));
    string::fmt_string(f, *value, StringKind::Docstring)?;
    let trailing = comments.trailing(statement);
    if kind == SuiteKind::Class
        && let Some(own_line) = trailing.iter().find(|comment| comment.is_own_line())
        && lines_before(f.source, own_line.span.start) < EMPTY_LINE
    {
        f.line(LineMode::Empty);
    }
    trailing_comments(f, trailing);
    Ok(())
}

fn empty_lines(f: &mut Formatter<'_>, lines: usize, kind: SuiteKind) {
    match lines {
        0 | 1 => f.line(LineMode::Hard),
        EMPTY_LINE => f.line(LineMode::Empty),
        _ => {
            f.line(LineMode::Empty);
            if kind == SuiteKind::TopLevel {
                f.line(LineMode::Empty);
            }
        }
    }
}

// Una función de una línea (`def f(): ...`) seguida de otra función sin línea en blanco: quedan
// juntas, como las sobrecargas.
fn is_stub_function_without_empty_line(f: &Formatter<'_>, preceding: NodeId, following: NodeId) -> bool {
    let Some(Stmt::FunctionDef { body, .. }) = f.ast.stmt(preceding) else {
        return false;
    };
    matches!(f.ast.stmt(following), Some(Stmt::FunctionDef { .. }))
        && as_only_an_ellipsis(f, body).is_some()
        && lines_after_ignoring_end_of_line_trivia(f.source, f.ast.end(preceding)) < EMPTY_LINE
        && !f.comments.has_trailing_own_line(preceding)
}

fn end_with_trailing_comments(f: &Formatter<'_>, statement: NodeId) -> usize {
    f.comments
        .trailing(statement)
        .last()
        .map_or_else(|| f.ast.end(statement), |comment| comment.span.end)
}

fn start_with_leading_comments(f: &Formatter<'_>, statement: NodeId) -> usize {
    f.comments
        .leading(statement)
        .first()
        .map_or_else(|| f.ast.start(statement), |comment| comment.span.start)
}

// Las líneas en blanco entre dos sentencias: dos alrededor de una función o una clase al nivel del
// módulo, una adentro, una después de los imports, y si no las que había, hasta dos.
fn separate(
    f: &mut Formatter<'_>,
    (preceding, following): (NodeId, NodeId),
    kind: SuiteKind,
    empty_line_after_docstring: bool,
) {
    let needs_empty_lines = is_class_or_function_definition(f, following)
        || trailing_function_or_class_def(f, preceding).is_some();
    if needs_empty_lines {
        if !is_stub_function_without_empty_line(f, preceding, following) {
            f.line(LineMode::Empty);
            if kind == SuiteKind::TopLevel {
                f.line(LineMode::Empty);
            }
        }
    } else if is_import_definition(f, preceding)
        && (!is_import_definition(f, following) || f.comments.has_leading(following))
    {
        let lines = lines_after(f.source, end_with_trailing_comments(f, preceding));
        f.line(LineMode::Empty);
        if kind == SuiteKind::TopLevel && lines > EMPTY_LINE {
            f.line(LineMode::Empty);
        }
    } else if is_compound_statement(f, preceding) {
        let lines = lines_before(f.source, start_with_leading_comments(f, following));
        empty_lines(f, lines, kind);
    } else if empty_line_after_docstring {
        f.line(LineMode::Empty);
    } else {
        let lines = lines_after(f.source, end_with_trailing_comments(f, preceding));
        empty_lines(f, lines, kind);
    }
}

fn fmt_first(f: &mut Formatter<'_>, first: NodeId, kind: SuiteKind) -> Formatted<bool> {
    let is_docstring = kind != SuiteKind::Other && is_docstring_statement(f, first);
    if kind == SuiteKind::Other && is_class_or_function_definition(f, first) && !f.comments.has_leading(first)
    {
        f.line(LineMode::Empty);
    }
    let allow_newline_after_block_open = kind == SuiteKind::Function && !is_docstring;
    if allow_newline_after_block_open && lines_before(f.source, start_with_leading_comments(f, first)) > 1 {
        f.line(LineMode::Empty);
    }
    if is_docstring {
        fmt_docstring(f, first, kind)?;
    } else {
        format_statement(f, first)?;
    }
    Ok(is_docstring && matches!(kind, SuiteKind::Class | SuiteKind::TopLevel))
}

// `FormatSuite` de ruff: las sentencias de un bloque con sus líneas en blanco.
pub fn fmt_suite(f: &mut Formatter<'_>, statements: &[NodeId], kind: SuiteKind) -> Formatted {
    let Some((first, rest)) = statements.split_first() else {
        return Ok(());
    };
    let position_of = |is_last: bool| {
        if is_last {
            TopLevelPosition::Last
        } else {
            TopLevelPosition::Other
        }
    };
    let node_level = if kind == SuiteKind::TopLevel {
        NodeLevel::TopLevel(position_of(rest.is_empty()))
    } else {
        NodeLevel::CompoundStatement
    };
    let saved_level = f.enter(node_level);
    f.indent_level = f.indent_level.saturating_add(1);
    let mut empty_line_after_docstring = fmt_first(f, *first, kind)?;
    let mut preceding = *first;
    for (index, following) in rest.iter().enumerate() {
        if kind == SuiteKind::TopLevel && index.saturating_add(1) == rest.len() {
            f.node_level = NodeLevel::TopLevel(TopLevelPosition::Last);
        }
        separate(f, (preceding, *following), kind, empty_line_after_docstring);
        format_statement(f, *following)?;
        preceding = *following;
        empty_line_after_docstring = false;
    }
    f.indent_level = f.indent_level.saturating_sub(1);
    f.node_level = saved_level;
    Ok(())
}
