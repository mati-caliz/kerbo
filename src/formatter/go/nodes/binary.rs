use super::super::ast::{Binary, Expr};
use super::super::printer::{Break, Printer, Whitespace};
use super::super::token::Token;
use super::expressions::{expr0, expr1, reduce_depth};

const ADDITION_LEVEL: i64 = 4;
const MULTIPLICATION_LEVEL: i64 = 5;
const NORMAL_DEPTH: i64 = 1;
const NORMAL_CUTOFF: i64 = 6;
const MIXED_CUTOFF: i64 = 5;
const COMPACT_CUTOFF: i64 = 4;

#[derive(Debug, Default, Clone, Copy)]
struct Walk {
    has_addition: bool,
    has_multiplication: bool,
    max_problem: i64,
}

// Port de `walkBinary` de go/printer: qué niveles de precedencia aparecen y si algún operando
// unario se pegaría al operador (`/*`, `&&`, `&^`, `++`, `--`).
fn walk_binary(binary: &Binary) -> Walk {
    let precedence = binary.op.precedence();
    let mut walk = Walk {
        has_addition: precedence == ADDITION_LEVEL,
        has_multiplication: precedence == MULTIPLICATION_LEVEL,
        max_problem: 0,
    };
    if let Expr::Binary(left) = &binary.x
        && left.op.precedence() >= precedence
    {
        merge(&mut walk, walk_binary(left));
    }
    if let Expr::Binary(right) = &binary.y {
        if right.op.precedence() > precedence {
            merge(&mut walk, walk_binary(right));
        }
    } else if matches!(binary.y, Expr::Star(_)) {
        if binary.op == Token::Quo {
            walk.max_problem = MULTIPLICATION_LEVEL;
        }
    } else if let Expr::Unary(unary) = &binary.y {
        let pair = (binary.op, unary.op);
        if matches!(pair, (Token::And, Token::And | Token::Xor)) {
            walk.max_problem = MULTIPLICATION_LEVEL;
        } else if matches!(pair, (Token::Add, Token::Add) | (Token::Sub, Token::Sub)) {
            walk.max_problem = walk.max_problem.max(ADDITION_LEVEL);
        }
    }
    walk
}

const fn merge(walk: &mut Walk, other: Walk) {
    walk.has_addition = walk.has_addition || other.has_addition;
    walk.has_multiplication = walk.has_multiplication || other.has_multiplication;
    if other.max_problem > walk.max_problem {
        walk.max_problem = other.max_problem;
    }
}

fn cutoff(binary: &Binary, depth: i64) -> i64 {
    let walk = walk_binary(binary);
    if walk.max_problem > 0 {
        return walk.max_problem.saturating_add(1);
    }
    let mixed = walk.has_addition && walk.has_multiplication;
    match (mixed, depth == NORMAL_DEPTH) {
        (true, true) => MIXED_CUTOFF,
        (false, true) => NORMAL_CUTOFF,
        (_, false) => COMPACT_CUTOFF,
    }
}

fn diff_precedence(x: &Expr, precedence: i64) -> i64 {
    if let Expr::Binary(binary) = x
        && binary.op.precedence() == precedence
    {
        0
    } else {
        1
    }
}

pub fn binary_expr(printer: &mut Printer<'_, '_>, binary: &Binary, prec1: i64, depth: i64) {
    let precedence = binary.op.precedence();
    if precedence < prec1 {
        printer.token(Token::Lparen);
        expr0(
            printer,
            &Expr::Binary(Box::new(binary.clone())),
            reduce_depth(depth),
        );
        printer.token(Token::Rparen);
        return;
    }
    let mut print_blank = precedence < cutoff(binary, depth);
    let mut ws = Whitespace::Indent;
    expr1(
        printer,
        &binary.x,
        precedence,
        depth.saturating_add(diff_precedence(&binary.x, precedence)),
    );
    if print_blank {
        printer.ws(Whitespace::Blank);
    }
    let x_line = printer.pos.line;
    let y_line = printer.line_for(binary.y.pos());
    printer.set_pos(binary.op_pos);
    printer.token(binary.op);
    if x_line != y_line && x_line > 0 && y_line > 0 {
        let breaks = printer.linebreak(Break {
            line: y_line,
            min: 1,
            ws,
            section: true,
        });
        if breaks > 0 {
            ws = Whitespace::Ignore;
            print_blank = false;
        }
    }
    if print_blank {
        printer.ws(Whitespace::Blank);
    }
    expr1(
        printer,
        &binary.y,
        precedence.saturating_add(1),
        depth.saturating_add(1),
    );
    if ws == Whitespace::Ignore {
        printer.ws(Whitespace::Unindent);
    }
}
