use super::clause::{ClauseComments, clause, clause_header};
use super::suite::SuiteKind;
use crate::formatter::python::ast::{NodeId, Other, Stmt};
use crate::formatter::python::builders::block_indent;
use crate::formatter::python::comments::format::leading_alternate_branch_comments;
use crate::formatter::python::context::{Formatted, Formatter, NodeLevel};
use crate::formatter::python::expression::{format_node_with, maybe_parenthesize_expression};
use crate::formatter::python::parentheses::Parenthesize;
use crate::formatter::python::pattern::maybe_parenthesize_pattern;

const MATCH: &str = "match";
const CASE: &str = "case";
const IF: &str = "if";
const MATCH_STATEMENT: &str = "sentencia match";

fn fmt_case(f: &mut Formatter<'_>, case: NodeId) -> Formatted {
    let Some(Other::MatchCase { pattern, guard, body }) = f.ast.other(case) else {
        return Err(f.unsupported(case, MATCH_STATEMENT));
    };
    let dangling = f.comments.dangling(case);
    format_node_with(f, case, |nested| {
        clause(
            nested,
            ClauseComments::trailing(dangling),
            (body, SuiteKind::Other),
            |header| {
                header.token(CASE);
                header.space();
                maybe_parenthesize_pattern(header, *pattern, case)?;
                if let Some(guard) = guard {
                    header.space();
                    header.token(IF);
                    header.space();
                    maybe_parenthesize_expression(header, *guard, case, Parenthesize::IfBreaksParenthesized)?;
                }
                Ok(())
            },
        )
    })
}

fn last_statement_of(f: &Formatter<'_>, case: NodeId) -> Option<NodeId> {
    match f.ast.other(case) {
        Some(Other::MatchCase { body, .. }) => body.last().copied(),
        Some(_) | None => None,
    }
}

pub fn fmt_match(f: &mut Formatter<'_>, statement: NodeId) -> Formatted {
    let Some(Stmt::Match { subject, cases }) = f.ast.stmt(statement) else {
        return Err(f.unsupported(statement, MATCH_STATEMENT));
    };
    let comments = f.comments;
    clause_header(f, comments.dangling(statement), |header| {
        header.token(MATCH);
        header.space();
        maybe_parenthesize_expression(header, *subject, statement, Parenthesize::IfBreaks)
    })?;
    let saved_level = f.enter(NodeLevel::CompoundStatement);
    let mut last_case: Option<NodeId> = None;
    for case in cases {
        let previous_last = last_case.and_then(|previous| last_statement_of(f, previous));
        block_indent(f, |nested| {
            if last_case.is_some() {
                leading_alternate_branch_comments(nested, comments.leading(*case), previous_last);
            }
            fmt_case(nested, *case)
        })?;
        last_case = Some(*case);
    }
    f.node_level = saved_level;
    Ok(())
}
