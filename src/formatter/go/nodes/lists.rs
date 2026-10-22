use super::super::ast::{Expr, Ident};
use super::super::position::{Pos, Position};
use super::super::printer::{Break, Printer, Whitespace};
use super::super::token::Token;
use super::Node;
use super::expressions::{expr, expr0};
use super::sizes::node_size;

const UNBOUNDED_SIZE: i64 = 1_000_000;
const SMALL_SIZE: i64 = 40;
const ALIGNMENT_RATIO: f64 = 2.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ListMode(u8);

impl ListMode {
    pub const PLAIN: Self = Self(0);
    pub const COMMA_TERMINATED: Self = Self(1);
    pub const NO_INDENT: Self = Self(2);

    const fn has(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ExprList<'list> {
    pub prev: Pos,
    pub list: &'list [Expr],
    pub depth: i64,
    pub mode: ListMode,
    pub next: Pos,
}

struct Alignment {
    size: i64,
    log2_sum: f64,
    count: u32,
}

pub fn ident_list(printer: &mut Printer<'_, '_>, list: &[Ident], indent: bool) {
    let exprs: Vec<Expr> = list.iter().cloned().map(Expr::Ident).collect();
    let mode = if indent {
        ListMode::PLAIN
    } else {
        ListMode::NO_INDENT
    };
    expr_list(
        printer,
        &ExprList {
            prev: Pos::NONE,
            list: &exprs,
            depth: 1,
            mode,
            next: Pos::NONE,
        },
    );
}

pub fn expr_list(printer: &mut Printer<'_, '_>, request: &ExprList<'_>) {
    let (Some(first), Some(last)) = (request.list.first(), request.list.last()) else {
        return;
    };
    let prev = printer.pos_for(request.prev);
    let line = printer.line_for(first.pos());
    let end_line = printer.line_for(last.end());
    if prev.is_valid() && prev.line == line && line == end_line {
        for (index, element) in request.list.iter().enumerate() {
            if index > 0 {
                printer.set_pos(element.pos());
                printer.token(Token::Comma);
                printer.ws(Whitespace::Blank);
            }
            expr0(printer, element, request.depth);
        }
        return;
    }
    multiline_list(printer, request, line);
}

fn multiline_list(printer: &mut Printer<'_, '_>, request: &ExprList<'_>, first_line: i64) {
    let prev = printer.pos_for(request.prev);
    let next = printer.pos_for(request.next);
    let mut ws = if request.mode.has(ListMode::NO_INDENT) {
        Whitespace::Ignore
    } else {
        Whitespace::Indent
    };
    let broke_before =
        prev.is_valid() && prev.line < first_line && printer.linebreak(section(first_line, ws, true)) > 0;
    if broke_before {
        ws = Whitespace::Ignore;
    }
    let mut prev_break: Option<usize> = broke_before.then_some(0);
    let mut alignment = Alignment {
        size: 0,
        log2_sum: 0.0,
        count: 0,
    };
    let mut prev_line = prev.line;
    for (index, element) in request.list.iter().enumerate() {
        let line = printer.line_for(element.pos());
        let prev_size = alignment.size;
        alignment.size = element_size(printer, element, prev.is_valid() && next.is_valid());
        let use_formfeed = breaks_alignment(&alignment, prev_size);
        let needs_linebreak = 0 < prev_line && prev_line < line;
        if index > 0 {
            if !needs_linebreak {
                printer.set_pos(element.pos());
            }
            printer.token(Token::Comma);
            let after_previous = prev_break.map_or(0, |previous| previous.saturating_add(1));
            let breaks = if needs_linebreak {
                printer.linebreak(section(line, ws, use_formfeed || after_previous < index))
            } else {
                0
            };
            if breaks > 0 {
                ws = Whitespace::Ignore;
                prev_break = Some(index);
            }
            if breaks > 1 {
                alignment.log2_sum = 0.0;
                alignment.count = 0;
            }
            if breaks == 0 {
                printer.ws(Whitespace::Blank);
            }
        }
        print_element(printer, request, element, needs_linebreak && alignment.size > 0);
        if alignment.size > 0 {
            alignment.log2_sum += log2ish(alignment.size);
            alignment.count = alignment.count.saturating_add(1);
        }
        prev_line = line;
    }
    finish_list(printer, request, next, ws);
}

const fn section(line: i64, ws: Whitespace, section: bool) -> Break {
    Break {
        line,
        min: 0,
        ws,
        section,
    }
}

fn element_size(printer: &mut Printer<'_, '_>, element: &Expr, has_layout: bool) -> i64 {
    let size = node_size(printer, Node::Expr(element), UNBOUNDED_SIZE);
    if size > UNBOUNDED_SIZE || !has_layout {
        return 0;
    }
    if let Expr::KeyValue(pair) = element {
        return node_size(printer, Node::Expr(&pair.key), UNBOUNDED_SIZE);
    }
    size
}

fn breaks_alignment(alignment: &Alignment, prev_size: i64) -> bool {
    if prev_size <= 0 || alignment.size <= 0 {
        return true;
    }
    if alignment.count == 0 || (prev_size <= SMALL_SIZE && alignment.size <= SMALL_SIZE) {
        return false;
    }
    let geomean = exp2ish(alignment.log2_sum / f64::from(alignment.count));
    let ratio = to_float(alignment.size) / geomean;
    ALIGNMENT_RATIO * ratio <= 1.0 || ALIGNMENT_RATIO <= ratio
}

fn print_element(printer: &mut Printer<'_, '_>, request: &ExprList<'_>, element: &Expr, aligned: bool) {
    if let Expr::KeyValue(pair) = element
        && request.list.len() > 1
        && aligned
    {
        expr(printer, &pair.key);
        printer.set_pos(pair.colon);
        printer.token(Token::Colon);
        printer.ws(Whitespace::Vtab);
        expr(printer, &pair.value);
        return;
    }
    expr0(printer, element, request.depth);
}

fn finish_list(printer: &mut Printer<'_, '_>, request: &ExprList<'_>, next: Position, ws: Whitespace) {
    let indented = ws == Whitespace::Ignore && !request.mode.has(ListMode::NO_INDENT);
    if request.mode.has(ListMode::COMMA_TERMINATED) && next.is_valid() && printer.pos.line < next.line {
        printer.token(Token::Comma);
        if indented {
            printer.ws(Whitespace::Unindent);
        }
        printer.ws(Whitespace::Formfeed);
        return;
    }
    if indented {
        printer.ws(Whitespace::Unindent);
    }
}

fn to_float(value: i64) -> f64 {
    f64::from(u32::try_from(value).unwrap_or(u32::MAX))
}

// Las aproximaciones de `math.go` de go/printer: el corte de la alineación depende de estos
// redondeos, así que se calculan igual.
fn log2ish(value: i64) -> f64 {
    let positive = u64::try_from(value).unwrap_or_default();
    let exponent = u64::BITS.saturating_sub(positive.leading_zeros());
    let mut fraction = to_float(value);
    for _ in 0..exponent {
        fraction /= 2.0;
    }
    2.0f64.mul_add(fraction - 1.0, f64::from(exponent))
}

fn exp2ish(value: f64) -> f64 {
    let whole = value.floor();
    let mut result = 1.0 + (value - whole);
    let mut remaining = whole;
    while remaining.total_cmp(&1.0).is_ge() {
        result *= 2.0;
        remaining -= 1.0;
    }
    result
}
