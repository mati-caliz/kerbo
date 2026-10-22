use std::collections::HashMap;

use super::super::ast::{
    BasicLit, Block, Decl, File, FuncDecl, GenDecl, ImportSpec, Spec, TypeSpec, ValueSpec,
};
use super::super::position::LineTable;
use super::super::printer::{Break, Mode, Printer, Style, Whitespace};
use super::super::token::Token;
use super::expressions::{basic_lit, expr, ident};
use super::lists::{ExprList, ListMode, expr_list, ident_list};
use super::signatures::{ParamMode, parameters, signature};
use super::sizes::{body_size, distance_from, num_lines};
use super::statements::{block, stmt};

const MAX_ONE_LINE_BODY: i64 = 100;
const VALUE_SPEC_COLUMNS: usize = 3;
const FUNC_KEYWORD_WITH_BLANK: i64 = 5;
const RAW_QUOTE: char = '`';
const ILLEGAL_IMPORT_CHARACTERS: &str = "!\"#$%&'()*,:;<=>?[\\]^{|}`\u{FFFD}";

pub fn file(file: &File, lines: &LineTable) -> Vec<u8> {
    let mut sizes = HashMap::new();
    let mut printer = Printer::new(lines, Style::Gofmt, &file.comments, &mut sizes);
    printer.set_pos(file.package);
    printer.token(Token::Package);
    printer.ws(Whitespace::Blank);
    ident(&mut printer, &file.name);
    decl_list(&mut printer, &file.decls);
    printer.ws(Whitespace::Newline);
    printer.finish()
}

const fn decl_token(decl: &Decl) -> Token {
    match decl {
        Decl::Gen(decl) => decl.tok,
        Decl::Func(_) => Token::Func,
    }
}

fn decl_list(printer: &mut Printer<'_, '_>, list: &[Decl]) {
    let mut tok = Token::Illegal;
    for decl in list {
        let previous = tok;
        tok = decl_token(decl);
        if !printer.output.is_empty() {
            let min = if previous == tok { 1 } else { 2 };
            let section = tok == Token::Func && num_lines(printer, decl.pos(), decl.end()) > 1;
            printer.linebreak(Break {
                line: printer.line_for(decl.pos()),
                min,
                ws: Whitespace::Ignore,
                section,
            });
        }
        match decl {
            Decl::Gen(decl) => gen_decl(printer, decl),
            Decl::Func(decl) => func_decl(printer, decl),
        }
    }
}

pub fn gen_decl(printer: &mut Printer<'_, '_>, decl: &GenDecl) {
    printer.set_pos(decl.tok_pos);
    printer.token(decl.tok);
    printer.ws(Whitespace::Blank);
    if !decl.lparen.is_valid() && decl.specs.len() == 1 {
        if let Some(single) = decl.specs.first() {
            spec(printer, single, 1, true);
        }
        return;
    }
    printer.set_pos(decl.lparen);
    printer.token(Token::Lparen);
    if !decl.specs.is_empty() {
        printer.ws(Whitespace::Indent);
        printer.ws(Whitespace::Formfeed);
        let aligned_values = decl.specs.len() > 1 && matches!(decl.tok, Token::Const | Token::Var);
        let keep_types = if aligned_values {
            keep_type_column(&decl.specs)
        } else {
            Vec::new()
        };
        let slot = printer.new_line_slot();
        for (index, current) in decl.specs.iter().enumerate() {
            if index > 0 {
                let section = printer.lines_from(printer.recorded_line(slot)) > 0;
                printer.linebreak(Break {
                    line: printer.line_for(current.pos()),
                    min: 1,
                    ws: Whitespace::Ignore,
                    section,
                });
            }
            printer.record_line(slot);
            match current {
                Spec::Value(value) if aligned_values => {
                    value_spec(printer, value, keep_types.get(index).copied().unwrap_or_default());
                }
                Spec::Import(_) | Spec::Value(_) | Spec::Type(_) => {
                    spec(printer, current, decl.specs.len(), false);
                }
            }
        }
        printer.ws(Whitespace::Unindent);
        printer.ws(Whitespace::Formfeed);
    }
    printer.set_pos(decl.rparen);
    printer.token(Token::Rparen);
}

// Port de `keepTypeColumn` de go/printer: en una tanda de especificaciones con valores, la columna
// del tipo se conserva si alguna lo declara.
fn keep_type_column(specs: &[Spec]) -> Vec<bool> {
    let mut keep = vec![false; specs.len()];
    let mut run_start: Option<usize> = None;
    let mut keep_type = false;
    for (index, current) in specs.iter().enumerate() {
        let Spec::Value(value) = current else {
            continue;
        };
        if value.values.is_empty() {
            if let Some(start) = run_start.take() {
                mark(&mut keep, start..index, keep_type);
            }
        } else if run_start.is_none() {
            run_start = Some(index);
            keep_type = false;
        }
        if value.typ.is_some() {
            keep_type = true;
        }
    }
    if let Some(start) = run_start {
        mark(&mut keep, start..specs.len(), keep_type);
    }
    keep
}

fn mark(keep: &mut [bool], range: std::ops::Range<usize>, keep_type: bool) {
    if keep_type {
        for slot in keep.iter_mut().take(range.end).skip(range.start) {
            *slot = true;
        }
    }
}

fn value_spec(printer: &mut Printer<'_, '_>, value: &ValueSpec, keep_type: bool) {
    ident_list(printer, &value.names, false);
    let mut extra_tabs = VALUE_SPEC_COLUMNS;
    if value.typ.is_some() || keep_type {
        printer.ws(Whitespace::Vtab);
        extra_tabs = extra_tabs.saturating_sub(1);
    }
    if let Some(typ) = &value.typ {
        expr(printer, typ);
    }
    if !value.values.is_empty() {
        printer.ws(Whitespace::Vtab);
        printer.token(Token::Assign);
        printer.ws(Whitespace::Blank);
        values(printer, &value.values);
        extra_tabs = extra_tabs.saturating_sub(1);
    }
    if value.comment.is_some() {
        for _ in 0..extra_tabs {
            printer.ws(Whitespace::Vtab);
        }
    }
}

fn values(printer: &mut Printer<'_, '_>, list: &[super::super::ast::Expr]) {
    expr_list(
        printer,
        &ExprList {
            prev: super::super::position::Pos::NONE,
            list,
            depth: 1,
            mode: ListMode::PLAIN,
            next: super::super::position::Pos::NONE,
        },
    );
}

fn spec(printer: &mut Printer<'_, '_>, current: &Spec, count: usize, indent: bool) {
    match current {
        Spec::Import(import) => import_spec(printer, import),
        Spec::Value(value) => {
            ident_list(printer, &value.names, indent);
            if let Some(typ) = &value.typ {
                printer.ws(Whitespace::Blank);
                expr(printer, typ);
            }
            if !value.values.is_empty() {
                printer.ws(Whitespace::Blank);
                printer.token(Token::Assign);
                printer.ws(Whitespace::Blank);
                values(printer, &value.values);
            }
        }
        Spec::Type(typ) => type_spec(printer, typ, count),
    }
}

fn import_spec(printer: &mut Printer<'_, '_>, import: &ImportSpec) {
    if let Some(name) = &import.name {
        ident(printer, name);
        printer.ws(Whitespace::Blank);
    }
    basic_lit(printer, &sanitized_import_path(&import.path));
    printer.set_pos(import.end_pos);
}

// Como `sanitizeImportPath` de go/printer: un camino válido entre comillas invertidas pasa a
// comillas dobles.
fn sanitized_import_path(path: &BasicLit) -> BasicLit {
    let Some(inner) = path
        .value
        .strip_prefix(RAW_QUOTE)
        .and_then(|rest| rest.strip_suffix(RAW_QUOTE))
    else {
        return path.clone();
    };
    let valid = !inner.is_empty()
        && inner.chars().all(|character| {
            !character.is_whitespace()
                && !character.is_control()
                && !ILLEGAL_IMPORT_CHARACTERS.contains(character)
        });
    if !valid {
        return path.clone();
    }
    BasicLit {
        value: format!("\"{inner}\""),
        ..path.clone()
    }
}

fn type_spec(printer: &mut Printer<'_, '_>, typ: &TypeSpec, count: usize) {
    ident(printer, &typ.name);
    if let Some(type_params) = &typ.type_params {
        parameters(printer, type_params, ParamMode::TypeParams);
    }
    printer.ws(if count == 1 {
        Whitespace::Blank
    } else {
        Whitespace::Vtab
    });
    if typ.assign.is_valid() {
        printer.token(Token::Assign);
        printer.ws(Whitespace::Blank);
    }
    expr(printer, &typ.typ);
}

pub fn func_body(printer: &mut Printer<'_, '_>, header_size: i64, separator: Whitespace, body: &Block) {
    let level = printer.level;
    printer.level = 0;
    if header_size.saturating_add(body_size(printer, body, MAX_ONE_LINE_BODY)) <= MAX_ONE_LINE_BODY {
        one_line_body(printer, separator, body);
    } else {
        if separator != Whitespace::Ignore {
            printer.ws(Whitespace::Blank);
        }
        block(printer, body, 1);
    }
    printer.level = level;
}

fn one_line_body(printer: &mut Printer<'_, '_>, separator: Whitespace, body: &Block) {
    printer.ws(separator);
    printer.set_pos(body.lbrace);
    printer.token(Token::Lbrace);
    if !body.list.is_empty() {
        printer.ws(Whitespace::Blank);
        for (index, statement) in body.list.iter().enumerate() {
            if index > 0 {
                printer.token(Token::Semicolon);
                printer.ws(Whitespace::Blank);
            }
            stmt(printer, statement, index == body.list.len().saturating_sub(1));
        }
        printer.ws(Whitespace::Blank);
    }
    printer.toggle(Mode::NO_EXTRA_LINEBREAK);
    printer.set_pos(body.rbrace);
    printer.token(Token::Rbrace);
    printer.toggle(Mode::NO_EXTRA_LINEBREAK);
}

fn func_decl(printer: &mut Printer<'_, '_>, decl: &FuncDecl) {
    printer.set_pos(decl.typ.pos());
    printer.token(Token::Func);
    printer.ws(Whitespace::Blank);
    let start_column = printer.out.column.saturating_sub(FUNC_KEYWORD_WITH_BLANK);
    if let Some(receiver) = &decl.recv {
        parameters(printer, receiver, ParamMode::Func);
        printer.ws(Whitespace::Blank);
    }
    ident(printer, &decl.name);
    signature(printer, &decl.typ);
    if let Some(body) = &decl.body {
        let header = distance_from(printer, decl.typ.pos(), start_column);
        func_body(printer, header, Whitespace::Vtab, body);
    }
}
