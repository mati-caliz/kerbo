use super::super::ast::{Expr, FieldList, FuncType};
use super::super::printer::{Break, Printer, Whitespace};
use super::super::token::Token;
use super::expressions::expr;
use super::lists::ident_list;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamMode {
    Func,
    FuncTypeParams,
    TypeParams,
}

pub fn parameters(printer: &mut Printer<'_, '_>, fields: &FieldList, mode: ParamMode) {
    let (open, close) = if mode == ParamMode::Func {
        (Token::Lparen, Token::Rparen)
    } else {
        (Token::Lbrack, Token::Rbrack)
    };
    printer.set_pos(fields.opening);
    printer.token(open);
    if !fields.list.is_empty() {
        let ws = parameter_entries(printer, fields, mode);
        if ws == Whitespace::Ignore {
            printer.ws(Whitespace::Unindent);
        }
    }
    printer.set_pos(fields.closing);
    printer.token(close);
}

fn parameter_entries(printer: &mut Printer<'_, '_>, fields: &FieldList, mode: ParamMode) -> Whitespace {
    let mut prev_line = printer.line_for(fields.opening);
    let mut ws = Whitespace::Indent;
    for (index, parameter) in fields.list.iter().enumerate() {
        let begin_line = printer.line_for(parameter.pos());
        let end_line = printer.line_for(parameter.end());
        let needs_linebreak = 0 < prev_line && prev_line < begin_line;
        if index > 0 {
            if !needs_linebreak {
                printer.set_pos(parameter.pos());
            }
            printer.token(Token::Comma);
        }
        let broke = needs_linebreak
            && printer.linebreak(Break {
                line: begin_line,
                min: 0,
                ws,
                section: true,
            }) > 0;
        if broke {
            ws = Whitespace::Ignore;
        } else if index > 0 {
            printer.ws(Whitespace::Blank);
        }
        if !parameter.names.is_empty() {
            ident_list(printer, &parameter.names, ws == Whitespace::Indent);
            printer.ws(Whitespace::Blank);
        }
        expr(printer, parameter.typ.without_parens());
        prev_line = end_line;
    }
    let closing = printer.line_for(fields.closing);
    if 0 < prev_line && prev_line < closing {
        printer.token(Token::Comma);
        printer.linebreak(Break {
            line: closing,
            min: 0,
            ws: Whitespace::Ignore,
            section: true,
        });
    } else if mode == ParamMode::TypeParams
        && fields.field_count() == 1
        && fields
            .list
            .first()
            .is_some_and(|field| combines_with_name(field.typ.without_parens()))
    {
        printer.token(Token::Comma);
    }
    ws
}

fn combines_with_name(x: &Expr) -> bool {
    match x {
        Expr::Star(star) => !is_type_elem(&star.x),
        Expr::Binary(binary) => combines_with_name(&binary.x) && !is_type_elem(&binary.y),
        Expr::Paren(paren) => !is_type_elem(&paren.x),
        Expr::Ident(_)
        | Expr::BasicLit(_)
        | Expr::Composite(_)
        | Expr::FuncLit(_)
        | Expr::Selector(_)
        | Expr::Index(_)
        | Expr::IndexList(_)
        | Expr::Slice(_)
        | Expr::TypeAssert(_)
        | Expr::Call(_)
        | Expr::Unary(_)
        | Expr::KeyValue(_)
        | Expr::Array(_)
        | Expr::Struct(_)
        | Expr::Func(_)
        | Expr::Interface(_)
        | Expr::Map(_)
        | Expr::Chan(_)
        | Expr::Ellipsis(_) => false,
    }
}

fn is_type_elem(x: &Expr) -> bool {
    match x {
        Expr::Array(_)
        | Expr::Struct(_)
        | Expr::Func(_)
        | Expr::Interface(_)
        | Expr::Map(_)
        | Expr::Chan(_) => true,
        Expr::Unary(unary) => unary.op == Token::Tilde,
        Expr::Binary(binary) => is_type_elem(&binary.x) || is_type_elem(&binary.y),
        Expr::Paren(paren) => is_type_elem(&paren.x),
        Expr::Ident(_)
        | Expr::BasicLit(_)
        | Expr::Composite(_)
        | Expr::FuncLit(_)
        | Expr::Selector(_)
        | Expr::Index(_)
        | Expr::IndexList(_)
        | Expr::Slice(_)
        | Expr::TypeAssert(_)
        | Expr::Call(_)
        | Expr::Star(_)
        | Expr::KeyValue(_)
        | Expr::Ellipsis(_) => false,
    }
}

pub fn signature(printer: &mut Printer<'_, '_>, func: &FuncType) {
    if let Some(type_params) = &func.type_params {
        parameters(printer, type_params, ParamMode::FuncTypeParams);
    }
    parameters(printer, &func.params, ParamMode::Func);
    let Some(results) = &func.results else {
        return;
    };
    let count = results.field_count();
    if count == 0 {
        return;
    }
    printer.ws(Whitespace::Blank);
    if let [single] = results.list.as_slice()
        && count == 1
        && single.names.is_empty()
    {
        expr(printer, single.typ.without_parens());
        return;
    }
    parameters(printer, results, ParamMode::Func);
}
