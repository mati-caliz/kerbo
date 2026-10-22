use super::super::ast::{Expr, Field, FieldList, Ident};
use super::super::position::signed;
use super::super::printer::comments::comment_before;
use super::super::printer::{Break, Printer, Whitespace};
use super::super::token::Token;
use super::Node;
use super::expressions::{basic_lit, expr, ident};
use super::lists::ident_list;
use super::signatures::signature;
use super::sizes::node_size;

const ONE_LINE_MAX_SIZE: i64 = 30;
const IDENT_SEPARATOR_SIZE: i64 = 2;
const NAMED_FIELD_TABS: usize = 1;
const EMBEDDED_FIELD_TABS: usize = 2;

pub fn field_list(printer: &mut Printer<'_, '_>, fields: &FieldList, is_struct: bool) {
    let has_comments = comment_before(printer, printer.pos_for(fields.closing));
    let source_is_one_line = fields.opening.is_valid()
        && fields.closing.is_valid()
        && printer.line_for(fields.opening) == printer.line_for(fields.closing);
    if !has_comments && source_is_one_line && one_line(printer, fields, is_struct) {
        return;
    }
    printer.ws(Whitespace::Blank);
    printer.set_pos(fields.opening);
    printer.token(Token::Lbrace);
    printer.ws(Whitespace::Indent);
    if has_comments || !fields.list.is_empty() {
        printer.ws(Whitespace::Formfeed);
    }
    if is_struct {
        struct_fields(printer, &fields.list);
    } else {
        interface_elements(printer, &fields.list);
    }
    printer.ws(Whitespace::Unindent);
    printer.ws(Whitespace::Formfeed);
    printer.set_pos(fields.closing);
    printer.token(Token::Rbrace);
}

fn one_line(printer: &mut Printer<'_, '_>, fields: &FieldList, is_struct: bool) -> bool {
    if fields.list.is_empty() {
        printer.set_pos(fields.opening);
        printer.token(Token::Lbrace);
        printer.set_pos(fields.closing);
        printer.token(Token::Rbrace);
        return true;
    }
    let Some(field) = one_line_field(printer, &fields.list) else {
        return false;
    };
    printer.set_pos(fields.opening);
    printer.token(Token::Lbrace);
    printer.ws(Whitespace::Blank);
    if is_struct {
        for (index, name) in field.names.iter().enumerate() {
            if index > 0 {
                printer.token(Token::Comma);
                printer.ws(Whitespace::Blank);
            }
            ident(printer, name);
        }
        if !field.names.is_empty() {
            printer.ws(Whitespace::Blank);
        }
        expr(printer, &field.typ);
    } else {
        interface_element(printer, field);
    }
    printer.ws(Whitespace::Blank);
    printer.set_pos(fields.closing);
    printer.token(Token::Rbrace);
    true
}

fn one_line_field<'list>(printer: &mut Printer<'_, '_>, list: &'list [Field]) -> Option<&'list Field> {
    let [field] = list else {
        return None;
    };
    if field.tag.is_some() || field.comment.is_some() {
        return None;
    }
    let mut names_size = ident_list_size(&field.names, ONE_LINE_MAX_SIZE);
    if names_size > 0 {
        names_size = 1;
    }
    let type_size = node_size(printer, Node::Expr(&field.typ), ONE_LINE_MAX_SIZE);
    (names_size.saturating_add(type_size) <= ONE_LINE_MAX_SIZE).then_some(field)
}

fn ident_list_size(list: &[Ident], max_size: i64) -> i64 {
    let mut size: i64 = 0;
    for (index, ident) in list.iter().enumerate() {
        if index > 0 {
            size = size.saturating_add(IDENT_SEPARATOR_SIZE);
        }
        size = size.saturating_add(signed(ident.name.chars().count()));
        if size >= max_size {
            break;
        }
    }
    size
}

fn struct_fields(printer: &mut Printer<'_, '_>, list: &[Field]) {
    let separator = if list.len() == 1 {
        Whitespace::Blank
    } else {
        Whitespace::Vtab
    };
    let slot = printer.new_line_slot();
    for (index, field) in list.iter().enumerate() {
        if index > 0 {
            let section = printer.lines_from(printer.recorded_line(slot)) > 0;
            printer.linebreak(Break {
                line: printer.line_for(field.pos()),
                min: 1,
                ws: Whitespace::Ignore,
                section,
            });
        }
        printer.record_line(slot);
        let untagged_tabs = if field.names.is_empty() {
            expr(printer, &field.typ);
            EMBEDDED_FIELD_TABS
        } else {
            ident_list(printer, &field.names, false);
            printer.ws(separator);
            expr(printer, &field.typ);
            NAMED_FIELD_TABS
        };
        let extra_tabs = field.tag.as_ref().map_or(untagged_tabs, |tag| {
            if !field.names.is_empty() && separator == Whitespace::Vtab {
                printer.ws(separator);
            }
            printer.ws(separator);
            basic_lit(printer, tag);
            0
        });
        if field.comment.is_some() {
            for _ in 0..extra_tabs {
                printer.ws(separator);
            }
        }
    }
}

fn interface_elements(printer: &mut Printer<'_, '_>, list: &[Field]) {
    let slot = printer.new_line_slot();
    for (index, field) in list.iter().enumerate() {
        if index > 0 {
            let section = printer.lines_from(printer.recorded_line(slot)) > 0;
            printer.linebreak(Break {
                line: printer.line_for(field.pos()),
                min: 1,
                ws: Whitespace::Ignore,
                section,
            });
        }
        printer.record_line(slot);
        interface_element(printer, field);
    }
}

fn interface_element(printer: &mut Printer<'_, '_>, field: &Field) {
    if let (Some(name), Expr::Func(method)) = (field.names.first(), &field.typ) {
        ident(printer, name);
        signature(printer, method);
        return;
    }
    expr(printer, &field.typ);
}
