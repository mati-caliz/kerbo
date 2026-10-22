use super::super::ast::{
    AssignStmt, Block, BranchStmt, CallStmt, CaseClause, CommClause, Expr, IfStmt, IncDecStmt, LabeledStmt,
    RangeStmt, ReturnStmt, SelectStmt, SendStmt, Stmt, TypeSwitchStmt,
};
use super::super::position::Pos;
use super::super::printer::comments::comment_before;
use super::super::printer::{Break, Printer, Whitespace};
use super::super::token::Token;
use super::declarations::gen_decl;
use super::expressions::{expr, expr0, ident};
use super::lists::{ExprList, ListMode, expr_list};
use super::walk::{is_composite_like, strip_parens};

const INDENTED: i64 = 1;
const NOT_INDENTED: i64 = 0;

pub fn stmt_list(printer: &mut Printer<'_, '_>, list: &[Stmt], indent: i64, next_is_rbrace: bool) {
    if indent > 0 {
        printer.ws(Whitespace::Indent);
    }
    let slot = printer.new_line_slot();
    let mut index = 0;
    for statement in list {
        if matches!(statement, Stmt::Empty(_)) {
            continue;
        }
        if !printer.output.is_empty() {
            let section = index == 0 || indent == 0 || printer.lines_from(printer.recorded_line(slot)) > 0;
            printer.linebreak(Break {
                line: printer.line_for(statement.pos()),
                min: 1,
                ws: Whitespace::Ignore,
                section,
            });
        }
        printer.record_line(slot);
        stmt(
            printer,
            statement,
            next_is_rbrace && index == list.len().saturating_sub(1),
        );
        let mut current = statement;
        while let Stmt::Labeled(labeled) = current {
            printer.bump_recorded_line(slot);
            current = &labeled.stmt;
        }
        index = index.saturating_add(1);
    }
    if indent > 0 {
        printer.ws(Whitespace::Unindent);
    }
}

pub fn block(printer: &mut Printer<'_, '_>, body: &Block, indent: i64) {
    printer.set_pos(body.lbrace);
    printer.token(Token::Lbrace);
    stmt_list(printer, &body.list, indent, true);
    printer.linebreak(Break {
        line: printer.line_for(body.rbrace),
        min: 1,
        ws: Whitespace::Ignore,
        section: true,
    });
    printer.set_pos(body.rbrace);
    printer.token(Token::Rbrace);
}

struct Clause<'clause> {
    is_for: bool,
    init: Option<&'clause Stmt>,
    cond: Option<&'clause Expr>,
    post: Option<&'clause Stmt>,
}

fn control_clause(printer: &mut Printer<'_, '_>, clause: &Clause<'_>) {
    printer.ws(Whitespace::Blank);
    let mut needs_blank = false;
    if clause.init.is_none() && clause.post.is_none() {
        if let Some(cond) = clause.cond {
            expr(printer, strip_parens(cond));
            needs_blank = true;
        }
    } else {
        if let Some(init) = clause.init {
            stmt(printer, init, false);
        }
        printer.token(Token::Semicolon);
        printer.ws(Whitespace::Blank);
        if let Some(cond) = clause.cond {
            expr(printer, strip_parens(cond));
            needs_blank = true;
        }
        if clause.is_for {
            printer.token(Token::Semicolon);
            printer.ws(Whitespace::Blank);
            needs_blank = false;
            if let Some(post) = clause.post {
                stmt(printer, post, false);
                needs_blank = true;
            }
        }
    }
    if needs_blank {
        printer.ws(Whitespace::Blank);
    }
}

fn indent_list(printer: &Printer<'_, '_>, list: &[Expr]) -> bool {
    let (Some(first), Some(last)) = (list.first(), list.last()) else {
        return false;
    };
    if list.len() < 2 {
        return false;
    }
    let begin = printer.line_for(first.pos());
    let end = printer.line_for(last.end());
    if !(0 < begin && begin < end) {
        return false;
    }
    let mut multiline: usize = 0;
    let mut line = begin;
    for element in list {
        let element_begin = printer.line_for(element.pos());
        let element_end = printer.line_for(element.end());
        if line < element_begin {
            return true;
        }
        if element_begin < element_end && !is_composite_like(element) {
            multiline = multiline.saturating_add(1);
        }
        line = element_end;
    }
    multiline > 1
}

pub fn stmt(printer: &mut Printer<'_, '_>, statement: &Stmt, next_is_rbrace: bool) {
    printer.set_pos(statement.pos());
    match statement {
        Stmt::Decl(decl) => gen_decl(printer, decl),
        Stmt::Empty(_) => {}
        Stmt::Labeled(labeled) => labeled_stmt(printer, labeled, next_is_rbrace),
        Stmt::Expr(x) => expr0(printer, x, 1),
        Stmt::Send(send) => send_stmt(printer, send),
        Stmt::IncDec(statement) => inc_dec(printer, statement),
        Stmt::Assign(assign) => assign_stmt(printer, assign),
        Stmt::Go(statement) => call_stmt(printer, Token::Go, statement),
        Stmt::Defer(statement) => call_stmt(printer, Token::Defer, statement),
        Stmt::Return(statement) => return_stmt(printer, statement),
        Stmt::Branch(branch) => branch_stmt(printer, branch),
        Stmt::Block(body) => block(printer, body, INDENTED),
        Stmt::If(statement) => if_stmt(printer, statement, next_is_rbrace),
        Stmt::Case(clause) => case_clause(printer, clause, next_is_rbrace),
        Stmt::Switch(statement) => {
            printer.token(Token::Switch);
            let clause = Clause {
                is_for: false,
                init: statement.init.as_ref(),
                cond: statement.tag.as_ref(),
                post: None,
            };
            control_clause(printer, &clause);
            block(printer, &statement.body, NOT_INDENTED);
        }
        Stmt::TypeSwitch(statement) => type_switch(printer, statement),
        Stmt::Comm(clause) => comm_clause(printer, clause, next_is_rbrace),
        Stmt::Select(statement) => select_stmt(printer, statement),
        Stmt::For(statement) => {
            printer.token(Token::For);
            let clause = Clause {
                is_for: true,
                init: statement.init.as_ref(),
                cond: statement.cond.as_ref(),
                post: statement.post.as_ref(),
            };
            control_clause(printer, &clause);
            block(printer, &statement.body, INDENTED);
        }
        Stmt::Range(statement) => range_stmt(printer, statement),
    }
}

fn labeled_stmt(printer: &mut Printer<'_, '_>, labeled: &LabeledStmt, next_is_rbrace: bool) {
    printer.ws(Whitespace::Unindent);
    ident(printer, &labeled.label);
    printer.set_pos(labeled.colon);
    printer.token(Token::Colon);
    printer.ws(Whitespace::Indent);
    if let Stmt::Empty(pos) = labeled.stmt {
        if !next_is_rbrace {
            printer.ws(Whitespace::Newline);
            printer.set_pos(pos);
            printer.token(Token::Semicolon);
            return;
        }
    } else {
        printer.linebreak(Break {
            line: printer.line_for(labeled.stmt.pos()),
            min: 1,
            ws: Whitespace::Ignore,
            section: true,
        });
    }
    stmt(printer, &labeled.stmt, next_is_rbrace);
}

fn send_stmt(printer: &mut Printer<'_, '_>, send: &SendStmt) {
    expr0(printer, &send.chan, 1);
    printer.ws(Whitespace::Blank);
    printer.set_pos(send.arrow);
    printer.token(Token::Arrow);
    printer.ws(Whitespace::Blank);
    expr0(printer, &send.value, 1);
}

fn inc_dec(printer: &mut Printer<'_, '_>, statement: &IncDecStmt) {
    expr0(printer, &statement.x, 2);
    printer.set_pos(statement.tok_pos);
    printer.token(statement.tok);
}

fn assign_stmt(printer: &mut Printer<'_, '_>, assign: &AssignStmt) {
    let depth = if assign.lhs.len() > 1 && assign.rhs.len() > 1 {
        2
    } else {
        1
    };
    let start = assign.lhs.first().map_or(Pos::NONE, Expr::pos);
    expr_list(
        printer,
        &ExprList {
            prev: start,
            list: &assign.lhs,
            depth,
            mode: ListMode::PLAIN,
            next: assign.tok_pos,
        },
    );
    printer.ws(Whitespace::Blank);
    printer.set_pos(assign.tok_pos);
    printer.token(assign.tok);
    printer.ws(Whitespace::Blank);
    expr_list(
        printer,
        &ExprList {
            prev: assign.tok_pos,
            list: &assign.rhs,
            depth,
            mode: ListMode::PLAIN,
            next: Pos::NONE,
        },
    );
}

fn call_stmt(printer: &mut Printer<'_, '_>, keyword: Token, statement: &CallStmt) {
    printer.token(keyword);
    printer.ws(Whitespace::Blank);
    expr(printer, &statement.call);
}

fn return_stmt(printer: &mut Printer<'_, '_>, statement: &ReturnStmt) {
    printer.token(Token::Return);
    if statement.results.is_empty() {
        return;
    }
    printer.ws(Whitespace::Blank);
    let indented = indent_list(printer, &statement.results);
    if indented {
        printer.ws(Whitespace::Indent);
    }
    expr_list(
        printer,
        &ExprList {
            prev: Pos::NONE,
            list: &statement.results,
            depth: 1,
            mode: if indented {
                ListMode::NO_INDENT
            } else {
                ListMode::PLAIN
            },
            next: Pos::NONE,
        },
    );
    if indented {
        printer.ws(Whitespace::Unindent);
    }
}

fn branch_stmt(printer: &mut Printer<'_, '_>, branch: &BranchStmt) {
    printer.token(branch.tok);
    if let Some(label) = &branch.label {
        printer.ws(Whitespace::Blank);
        ident(printer, label);
    }
}

fn if_stmt(printer: &mut Printer<'_, '_>, statement: &IfStmt, next_is_rbrace: bool) {
    printer.token(Token::If);
    let clause = Clause {
        is_for: false,
        init: statement.init.as_ref(),
        cond: Some(&statement.cond),
        post: None,
    };
    control_clause(printer, &clause);
    block(printer, &statement.body, INDENTED);
    if let Some(alternative) = &statement.alternative {
        printer.ws(Whitespace::Blank);
        printer.token(Token::Else);
        printer.ws(Whitespace::Blank);
        stmt(printer, alternative, next_is_rbrace);
    }
}

fn case_clause(printer: &mut Printer<'_, '_>, clause: &CaseClause, next_is_rbrace: bool) {
    if let Some(list) = &clause.list {
        printer.token(Token::Case);
        printer.ws(Whitespace::Blank);
        expr_list(
            printer,
            &ExprList {
                prev: clause.keyword,
                list,
                depth: 1,
                mode: ListMode::PLAIN,
                next: clause.colon,
            },
        );
    } else {
        printer.token(Token::Default);
    }
    printer.set_pos(clause.colon);
    printer.token(Token::Colon);
    stmt_list(printer, &clause.body, INDENTED, next_is_rbrace);
}

fn type_switch(printer: &mut Printer<'_, '_>, statement: &TypeSwitchStmt) {
    printer.token(Token::Switch);
    if let Some(init) = &statement.init {
        printer.ws(Whitespace::Blank);
        stmt(printer, init, false);
        printer.token(Token::Semicolon);
    }
    printer.ws(Whitespace::Blank);
    stmt(printer, &statement.assign, false);
    printer.ws(Whitespace::Blank);
    block(printer, &statement.body, NOT_INDENTED);
}

fn comm_clause(printer: &mut Printer<'_, '_>, clause: &CommClause, next_is_rbrace: bool) {
    if let Some(comm) = &clause.comm {
        printer.token(Token::Case);
        printer.ws(Whitespace::Blank);
        stmt(printer, comm, false);
    } else {
        printer.token(Token::Default);
    }
    printer.set_pos(clause.colon);
    printer.token(Token::Colon);
    stmt_list(printer, &clause.body, INDENTED, next_is_rbrace);
}

fn select_stmt(printer: &mut Printer<'_, '_>, statement: &SelectStmt) {
    printer.token(Token::Select);
    printer.ws(Whitespace::Blank);
    let body = &statement.body;
    if body.list.is_empty() && !comment_before(printer, printer.pos_for(body.rbrace)) {
        printer.set_pos(body.lbrace);
        printer.token(Token::Lbrace);
        printer.set_pos(body.rbrace);
        printer.token(Token::Rbrace);
        return;
    }
    block(printer, body, NOT_INDENTED);
}

fn range_stmt(printer: &mut Printer<'_, '_>, statement: &RangeStmt) {
    printer.token(Token::For);
    printer.ws(Whitespace::Blank);
    if let Some(key) = &statement.key {
        expr(printer, key);
        if let Some(value) = &statement.value {
            printer.set_pos(value.pos());
            printer.token(Token::Comma);
            printer.ws(Whitespace::Blank);
            expr(printer, value);
        }
        printer.ws(Whitespace::Blank);
        printer.set_pos(statement.tok_pos);
        printer.token(statement.tok);
        printer.ws(Whitespace::Blank);
    }
    printer.token(Token::Range);
    printer.ws(Whitespace::Blank);
    expr(printer, strip_parens(&statement.x));
    printer.ws(Whitespace::Blank);
    block(printer, &statement.body, INDENTED);
}
