use super::super::ast::{
    BasicLit, Call, ChanDir, ChanType, CompositeLit, Expr, FuncLit, Ident, Index, IndexList, Paren, Selector,
    Star, TypeAssert, Unary,
};
use super::super::printer::{Mode, Printer, Style, Whitespace};
use super::super::token::{HIGHEST_PRECEDENCE, LOWEST_PRECEDENCE, Token, UNARY_PRECEDENCE};
use super::binary::binary_expr;
use super::declarations::func_body;
use super::fields::field_list;
use super::lists::{ExprList, ListMode, expr_list};
use super::numbers::normalized;
use super::signatures::signature;
use super::sizes::distance_from;
use super::slices::slice_expr;

const FUNC_KEYWORD_LENGTH: i64 = 4;

pub fn expr(printer: &mut Printer<'_, '_>, x: &Expr) {
    expr1(printer, x, LOWEST_PRECEDENCE, 1);
}

pub fn expr0(printer: &mut Printer<'_, '_>, x: &Expr, depth: i64) {
    expr1(printer, x, LOWEST_PRECEDENCE, depth);
}

pub fn ident(printer: &mut Printer<'_, '_>, name: &Ident) {
    printer.set_pos(name.pos);
    printer.ident(&name.name);
}

pub fn basic_lit(printer: &mut Printer<'_, '_>, literal: &BasicLit) {
    printer.set_pos(literal.pos);
    print_literal(printer, literal);
}

fn print_literal(printer: &mut Printer<'_, '_>, literal: &BasicLit) {
    if printer.style == Style::Gofmt {
        let value = normalized(literal.kind, &literal.value);
        printer.literal(literal.kind, &value);
    } else {
        printer.literal(literal.kind, &literal.value);
    }
}

pub fn expr1(printer: &mut Printer<'_, '_>, x: &Expr, prec1: i64, depth: i64) {
    printer.set_pos(x.pos());
    match x {
        Expr::Ident(name) => printer.ident(&name.name),
        Expr::BasicLit(literal) => print_literal(printer, literal),
        Expr::Binary(binary) => binary_expr(printer, binary, prec1, depth.max(1)),
        Expr::KeyValue(pair) => {
            expr(printer, &pair.key);
            printer.set_pos(pair.colon);
            printer.token(Token::Colon);
            printer.ws(Whitespace::Blank);
            expr(printer, &pair.value);
        }
        Expr::Star(star) => star_expr(printer, star, prec1),
        Expr::Unary(unary) => unary_expr(printer, unary, prec1, depth),
        Expr::FuncLit(literal) => func_lit(printer, literal),
        Expr::Paren(paren) => paren_expr(printer, paren, depth),
        Expr::Selector(selector) => {
            selector_expr(printer, selector, depth, false);
        }
        Expr::TypeAssert(assert) => type_assert(printer, assert, depth),
        Expr::Index(index) => index_expr(printer, index, depth),
        Expr::IndexList(index) => index_list(printer, index, depth),
        Expr::Slice(slice) => slice_expr(printer, slice, depth),
        Expr::Call(call) => call_expr(printer, call, depth),
        Expr::Composite(literal) => composite(printer, literal, depth),
        Expr::Ellipsis(ellipsis) => {
            printer.token(Token::Ellipsis);
            if let Some(elt) = &ellipsis.elt {
                expr(printer, elt);
            }
        }
        Expr::Array(array) => {
            printer.token(Token::Lbrack);
            if let Some(len) = &array.len {
                expr(printer, len);
            }
            printer.token(Token::Rbrack);
            expr(printer, &array.elt);
        }
        Expr::Struct(structure) => {
            printer.token(Token::Struct);
            field_list(printer, &structure.fields, true);
        }
        Expr::Func(func) => {
            printer.token(Token::Func);
            signature(printer, func);
        }
        Expr::Interface(interface) => {
            printer.token(Token::Interface);
            field_list(printer, &interface.methods, false);
        }
        Expr::Map(map) => {
            printer.token(Token::Map);
            printer.token(Token::Lbrack);
            expr(printer, &map.key);
            printer.token(Token::Rbrack);
            expr(printer, &map.value);
        }
        Expr::Chan(chan) => chan_type(printer, chan),
    }
}

fn star_expr(printer: &mut Printer<'_, '_>, star: &Star, prec1: i64) {
    let parenthesized = UNARY_PRECEDENCE < prec1;
    if parenthesized {
        printer.token(Token::Lparen);
    }
    printer.token(Token::Mul);
    expr(printer, &star.x);
    if parenthesized {
        printer.token(Token::Rparen);
    }
}

fn unary_expr(printer: &mut Printer<'_, '_>, unary: &Unary, prec1: i64, depth: i64) {
    if UNARY_PRECEDENCE < prec1 {
        printer.token(Token::Lparen);
        expr(printer, &Expr::Unary(Box::new(unary.clone())));
        printer.token(Token::Rparen);
        return;
    }
    printer.token(unary.op);
    expr1(printer, &unary.x, UNARY_PRECEDENCE, depth);
}

fn func_lit(printer: &mut Printer<'_, '_>, literal: &FuncLit) {
    printer.set_pos(literal.typ.pos());
    printer.token(Token::Func);
    let start_column = printer.out.column.saturating_sub(FUNC_KEYWORD_LENGTH);
    signature(printer, &literal.typ);
    let header = distance_from(printer, literal.typ.pos(), start_column);
    func_body(printer, header, Whitespace::Blank, &literal.body);
}

fn paren_expr(printer: &mut Printer<'_, '_>, paren: &Paren, depth: i64) {
    if matches!(paren.x, Expr::Paren(_)) {
        expr0(printer, &paren.x, depth);
        return;
    }
    printer.token(Token::Lparen);
    expr0(printer, &paren.x, reduce_depth(depth));
    printer.set_pos(paren.rparen);
    printer.token(Token::Rparen);
}

pub const fn reduce_depth(depth: i64) -> i64 {
    if depth > 1 { depth - 1 } else { 1 }
}

fn type_assert(printer: &mut Printer<'_, '_>, assert: &TypeAssert, depth: i64) {
    expr1(printer, &assert.x, HIGHEST_PRECEDENCE, depth);
    printer.token(Token::Period);
    printer.set_pos(assert.lparen);
    printer.token(Token::Lparen);
    match &assert.typ {
        Some(typ) => expr(printer, typ),
        None => printer.token(Token::Type),
    }
    printer.set_pos(assert.rparen);
    printer.token(Token::Rparen);
}

fn index_expr(printer: &mut Printer<'_, '_>, index: &Index, depth: i64) {
    expr1(printer, &index.x, HIGHEST_PRECEDENCE, 1);
    printer.set_pos(index.lbrack);
    printer.token(Token::Lbrack);
    expr0(printer, &index.index, depth.saturating_add(1));
    printer.set_pos(index.rbrack);
    printer.token(Token::Rbrack);
}

fn index_list(printer: &mut Printer<'_, '_>, index: &IndexList, depth: i64) {
    expr1(printer, &index.x, HIGHEST_PRECEDENCE, 1);
    printer.set_pos(index.lbrack);
    printer.token(Token::Lbrack);
    expr_list(
        printer,
        &ExprList {
            prev: index.lbrack,
            list: &index.indices,
            depth: depth.saturating_add(1),
            mode: ListMode::COMMA_TERMINATED,
            next: index.rbrack,
        },
    );
    printer.set_pos(index.rbrack);
    printer.token(Token::Rbrack);
}

fn call_expr(printer: &mut Printer<'_, '_>, call: &Call, depth: i64) {
    let depth = if call.args.len() > 1 {
        depth.saturating_add(1)
    } else {
        depth
    };
    let parenthesized = matches!(&call.fun, Expr::Func(_))
        || matches!(&call.fun, Expr::Chan(chan) if chan.dir == ChanDir::Receive);
    if parenthesized {
        printer.token(Token::Lparen);
    }
    let was_indented = possible_selector_expr(printer, &call.fun, depth);
    if parenthesized {
        printer.token(Token::Rparen);
    }
    printer.set_pos(call.lparen);
    printer.token(Token::Lparen);
    if call.ellipsis.is_valid() {
        expr_list(
            printer,
            &ExprList {
                prev: call.lparen,
                list: &call.args,
                depth,
                mode: ListMode::PLAIN,
                next: call.ellipsis,
            },
        );
        printer.set_pos(call.ellipsis);
        printer.token(Token::Ellipsis);
        if call.rparen.is_valid() && printer.line_for(call.ellipsis) < printer.line_for(call.rparen) {
            printer.token(Token::Comma);
            printer.ws(Whitespace::Formfeed);
        }
    } else {
        expr_list(
            printer,
            &ExprList {
                prev: call.lparen,
                list: &call.args,
                depth,
                mode: ListMode::COMMA_TERMINATED,
                next: call.rparen,
            },
        );
    }
    printer.set_pos(call.rparen);
    printer.token(Token::Rparen);
    if was_indented {
        printer.ws(Whitespace::Unindent);
    }
}

fn composite(printer: &mut Printer<'_, '_>, literal: &CompositeLit, depth: i64) {
    if let Some(typ) = &literal.typ {
        expr1(printer, typ, HIGHEST_PRECEDENCE, depth);
    }
    printer.level = printer.level.saturating_add(1);
    printer.set_pos(literal.lbrace);
    printer.token(Token::Lbrace);
    expr_list(
        printer,
        &ExprList {
            prev: literal.lbrace,
            list: &literal.elts,
            depth: 1,
            mode: ListMode::COMMA_TERMINATED,
            next: literal.rbrace,
        },
    );
    let mut mode = Mode::NO_EXTRA_LINEBREAK;
    if !literal.elts.is_empty() {
        mode = mode.union(Mode::NO_EXTRA_BLANK);
    }
    printer.ws(Whitespace::Indent);
    printer.ws(Whitespace::Unindent);
    printer.toggle(mode);
    printer.set_pos(literal.rbrace);
    printer.token(Token::Rbrace);
    printer.toggle(mode);
    printer.level = printer.level.saturating_sub(1);
}

fn chan_type(printer: &mut Printer<'_, '_>, chan: &ChanType) {
    match chan.dir {
        ChanDir::Both => printer.token(Token::Chan),
        ChanDir::Receive => {
            printer.token(Token::Arrow);
            printer.token(Token::Chan);
        }
        ChanDir::Send => {
            printer.token(Token::Chan);
            printer.set_pos(chan.arrow);
            printer.token(Token::Arrow);
        }
    }
    printer.ws(Whitespace::Blank);
    expr(printer, &chan.value);
}

fn possible_selector_expr(printer: &mut Printer<'_, '_>, x: &Expr, depth: i64) -> bool {
    if let Expr::Selector(selector) = x {
        return selector_expr(printer, selector, depth, true);
    }
    expr1(printer, x, HIGHEST_PRECEDENCE, depth);
    false
}

pub fn selector_expr(
    printer: &mut Printer<'_, '_>,
    selector: &Selector,
    depth: i64,
    is_method: bool,
) -> bool {
    expr1(printer, &selector.x, HIGHEST_PRECEDENCE, depth);
    printer.token(Token::Period);
    let line = printer.line_for(selector.sel.pos);
    if printer.pos.is_valid() && printer.pos.line < line {
        printer.ws(Whitespace::Indent);
        printer.ws(Whitespace::Newline);
        ident(printer, &selector.sel);
        if !is_method {
            printer.ws(Whitespace::Unindent);
        }
        return true;
    }
    ident(printer, &selector.sel);
    false
}
