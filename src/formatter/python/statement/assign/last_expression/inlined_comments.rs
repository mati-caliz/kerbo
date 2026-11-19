use crate::formatter::python::ast::NodeId;
use crate::formatter::python::comments::SourceComment;
use crate::formatter::python::comments::format::trailing_comments;
use crate::formatter::python::context::Formatter;

// Los comentarios de fin de línea del valor y de la sentencia: van después del valor si queda en
// una línea, o adentro de los paréntesis si hay que agregarlos.
#[derive(Debug, Clone, Copy, Default)]
pub struct InlinedComments<'comments> {
    expression: &'comments [SourceComment],
    statement: &'comments [SourceComment],
}

fn end_of_line_prefix(comments: &[SourceComment]) -> &[SourceComment] {
    let after_end_of_line = comments.partition_point(|comment| !comment.is_own_line());
    comments.get(..after_end_of_line).unwrap_or_default()
}

impl<'comments> InlinedComments<'comments> {
    pub fn new(f: &Formatter<'comments>, value: NodeId, statement: NodeId) -> Option<Self> {
        let comments = f.comments;
        if comments.has_leading(value) || comments.has_trailing_own_line(value) {
            return None;
        }
        Some(Self {
            expression: end_of_line_prefix(comments.trailing(value)),
            statement: end_of_line_prefix(comments.trailing(statement)),
        })
    }

    pub const fn is_empty(&self) -> bool {
        self.expression.is_empty() && self.statement.is_empty()
    }

    pub fn mark_formatted(&self) {
        self.expression.iter().for_each(SourceComment::mark_formatted);
    }

    pub fn write(&self, f: &mut Formatter<'_>) {
        for comment in self.expression.iter().chain(self.statement) {
            comment.mark_unformatted();
        }
        trailing_comments(f, self.expression);
        trailing_comments(f, self.statement);
    }
}
