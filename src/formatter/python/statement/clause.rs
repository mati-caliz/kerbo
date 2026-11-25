use super::format_statement;
use super::suite::{SuiteKind, as_only_an_ellipsis, fmt_suite};
use crate::formatter::document::LineMode;
use crate::formatter::python::ast::NodeId;
use crate::formatter::python::builders::block_indent;
use crate::formatter::python::comments::SourceComment;
use crate::formatter::python::comments::format::{leading_alternate_branch_comments, trailing_comments};
use crate::formatter::python::context::{Formatted, Formatter};

const COLON: &str = ":";

// Lo que rodea al encabezado de una cláusula: los comentarios de después de los dos puntos y, en
// un `else` o un `except`, los de antes junto con la última sentencia del bloque anterior.
#[derive(Debug, Clone, Copy)]
pub struct ClauseComments<'comments> {
    pub leading: Option<(&'comments [SourceComment], Option<NodeId>)>,
    pub trailing_colon: &'comments [SourceComment],
}

impl<'comments> ClauseComments<'comments> {
    pub const fn trailing(trailing_colon: &'comments [SourceComment]) -> Self {
        Self {
            leading: None,
            trailing_colon,
        }
    }

    pub const fn with_leading(
        leading: &'comments [SourceComment],
        last_node: Option<NodeId>,
        trailing_colon: &'comments [SourceComment],
    ) -> Self {
        Self {
            leading: Some((leading, last_node)),
            trailing_colon,
        }
    }
}

// Sólo el encabezado de una cláusula: lo que le sigue lo escribe quien la llama.
pub fn clause_header(
    f: &mut Formatter<'_>,
    trailing_colon: &[SourceComment],
    header: impl FnOnce(&mut Formatter<'_>) -> Formatted,
) -> Formatted {
    header(f)?;
    f.token(COLON);
    trailing_comments(f, trailing_colon);
    Ok(())
}

// `clause` de ruff: el encabezado (`if x`, `else`, `def f()`), los dos puntos y el bloque sangrado.
// Una función o una clase cuyo cuerpo es sólo `...` queda en una línea.
pub fn clause(
    f: &mut Formatter<'_>,
    comments: ClauseComments<'_>,
    (body, kind): (&[NodeId], SuiteKind),
    header: impl FnOnce(&mut Formatter<'_>) -> Formatted,
) -> Formatted {
    if let Some((leading, last_node)) = comments.leading {
        leading_alternate_branch_comments(f, leading, last_node);
    }
    header(f)?;
    f.token(COLON);
    trailing_comments(f, comments.trailing_colon);
    let collapses_stub = matches!(kind, SuiteKind::Function | SuiteKind::Class);
    if collapses_stub
        && comments.trailing_colon.is_empty()
        && let Some(ellipsis) = as_only_an_ellipsis(f, body)
    {
        f.space();
        format_statement(f, ellipsis)?;
        f.line(LineMode::Hard);
        return Ok(());
    }
    block_indent(f, |f| fmt_suite(f, body, kind))
}
