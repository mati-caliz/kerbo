use super::super::ast::{Expr, Slice};
use super::super::printer::{Printer, Whitespace};
use super::super::token::{HIGHEST_PRECEDENCE, Token};
use super::expressions::{expr0, expr1};

pub fn slice_expr(printer: &mut Printer<'_, '_>, slice: &Slice, depth: i64) {
    expr1(printer, &slice.x, HIGHEST_PRECEDENCE, 1);
    printer.set_pos(slice.lbrack);
    printer.token(Token::Lbrack);
    let mut indices = vec![slice.low.as_ref(), slice.high.as_ref()];
    if slice.max.is_some() {
        indices.push(slice.max.as_ref());
    }
    let needs_blanks = depth <= 1 && {
        let present: Vec<&Expr> = indices.iter().flatten().copied().collect();
        present.len() > 1 && present.iter().any(|index| matches!(index, Expr::Binary(_)))
    };
    let mut previous_present = false;
    for (position, index) in indices.iter().enumerate() {
        if position > 0 {
            if previous_present && needs_blanks {
                printer.ws(Whitespace::Blank);
            }
            printer.token(Token::Colon);
            if index.is_some() && needs_blanks {
                printer.ws(Whitespace::Blank);
            }
        }
        if let Some(index) = index {
            expr0(printer, index, depth.saturating_add(1));
        }
        previous_present = index.is_some();
    }
    printer.set_pos(slice.rbrack);
    printer.token(Token::Rbrack);
}
