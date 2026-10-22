use super::super::ast::{Block, Stmt};
use super::super::position::{INFINITY, Pos, signed};
use super::super::printer::comments::comment_size_before;
use super::super::printer::{Printer, Style};
use super::{Node, expressions, statements};

const MAX_ONE_LINE_STATEMENTS: usize = 5;
const STATEMENT_SEPARATOR_SIZE: i64 = 2;
const NEWLINE: u8 = b'\n';

pub fn node_size(printer: &mut Printer<'_, '_>, node: Node<'_>, max_size: i64) -> i64 {
    let key = node.key();
    if let Some(size) = printer.sizes.get(&key) {
        return *size;
    }
    let mut size = max_size.saturating_add(1);
    printer.sizes.insert(key, size);
    let lines = printer.lines;
    let mut child = Printer::new(lines, Style::Raw, &[], printer.sizes);
    print_node(&mut child, node);
    let output = child.finish();
    let length = signed(output.len());
    if length <= max_size && !output.contains(&NEWLINE) {
        size = length;
        printer.sizes.insert(key, size);
    }
    size
}

fn print_node(printer: &mut Printer<'_, '_>, node: Node<'_>) {
    match node {
        Node::Expr(expr) => expressions::expr(printer, expr),
        Node::Stmt(stmt) => {
            if matches!(stmt, Stmt::Labeled(_)) {
                printer.indent = 1;
            }
            statements::stmt(printer, stmt, false);
        }
    }
}

pub fn body_size(printer: &mut Printer<'_, '_>, block: &Block, max_size: i64) -> i64 {
    let opening = block.lbrace;
    let closing = block.rbrace;
    if opening.is_valid() && closing.is_valid() && printer.line_for(opening) != printer.line_for(closing) {
        return max_size.saturating_add(1);
    }
    if block.list.len() > MAX_ONE_LINE_STATEMENTS {
        return max_size.saturating_add(1);
    }
    let next = printer.pos_for(closing);
    let mut size = comment_size_before(printer, next);
    for (index, stmt) in block.list.iter().enumerate() {
        if size > max_size {
            break;
        }
        if index > 0 {
            size = size.saturating_add(STATEMENT_SEPARATOR_SIZE);
        }
        size = size.saturating_add(node_size(printer, Node::Stmt(stmt), max_size));
    }
    size
}

pub fn num_lines(printer: &Printer<'_, '_>, from: Pos, to: Pos) -> i64 {
    if from.is_valid() && to.is_valid() {
        return printer
            .line_for(to)
            .saturating_sub(printer.line_for(from))
            .saturating_add(1);
    }
    INFINITY
}

pub fn distance_from(printer: &Printer<'_, '_>, start: Pos, start_column: i64) -> i64 {
    if start.is_valid() && printer.pos.is_valid() && printer.line_for(start) == printer.pos.line {
        return printer.out.column.saturating_sub(start_column);
    }
    INFINITY
}
