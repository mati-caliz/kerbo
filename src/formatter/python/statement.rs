mod assign;
mod clause;
mod compound;
mod definition;
mod imports;
pub mod parameters;
mod simple;
pub mod suite;
mod with;

use super::ast::{NodeId, Stmt};
use super::context::{Formatted, Formatter};
use super::expression::format_node_with;

const STATEMENT: &str = "sentencia";

fn fmt_fields(f: &mut Formatter<'_>, statement: NodeId, data: &Stmt) -> Formatted {
    match data {
        Stmt::FunctionDef { .. } => definition::fmt_function(f, statement),
        Stmt::ClassDef { .. } => definition::fmt_class(f, statement),
        Stmt::Return { value } => simple::fmt_return(f, statement, *value),
        Stmt::Delete { targets } => simple::fmt_delete(f, statement, targets),
        Stmt::Assign { targets, value } => assign::fmt_assign(f, statement, targets, *value),
        Stmt::AugAssign { target, op, value } => simple::fmt_aug_assign(f, statement, (*target, *op), *value),
        Stmt::AnnAssign {
            target,
            annotation,
            value,
        } => simple::fmt_ann_assign(f, statement, (*target, *annotation), *value),
        Stmt::For { .. } => compound::fmt_for(f, statement),
        Stmt::While { test, body, orelse } => compound::fmt_while(f, statement, *test, (body, orelse)),
        Stmt::If { test, body, clauses } => compound::fmt_if(f, statement, *test, (body, clauses)),
        Stmt::With {
            is_async,
            items,
            body,
        } => with::fmt_with(f, statement, *is_async, (items, body)),
        Stmt::Raise { exc, cause } => simple::fmt_raise(f, statement, *exc, *cause),
        Stmt::Try { .. } => compound::fmt_try(f, statement),
        Stmt::Assert { test, msg } => simple::fmt_assert(f, statement, *test, *msg),
        Stmt::Import { names, is_lazy } => imports::fmt_import(f, names, *is_lazy),
        Stmt::ImportFrom { .. } => imports::fmt_import_from(f, statement),
        Stmt::Global { names } => simple::fmt_names(f, statement, "global", names),
        Stmt::Nonlocal { names } => simple::fmt_names(f, statement, "nonlocal", names),
        Stmt::Expression { value } => simple::fmt_expression(f, statement, *value),
        Stmt::Pass => {
            f.token("pass");
            Ok(())
        }
        Stmt::Break => {
            f.token("break");
            Ok(())
        }
        Stmt::Continue => {
            f.token("continue");
            Ok(())
        }
    }
}

pub fn format_statement(f: &mut Formatter<'_>, statement: NodeId) -> Formatted {
    let Some(data) = f.ast.stmt(statement) else {
        return Err(f.unsupported(statement, STATEMENT));
    };
    format_node_with(f, statement, |f| fmt_fields(f, statement, data))
}
