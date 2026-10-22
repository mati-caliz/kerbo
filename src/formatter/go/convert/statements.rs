use tree_sitter::Node;

use super::super::ast::{
    AssignStmt, Block, BranchStmt, CallStmt, CaseClause, CommClause, Expr, ForStmt, IfStmt, IncDecStmt,
    LabeledStmt, RangeStmt, ReturnStmt, SelectStmt, SendStmt, Stmt, SwitchStmt, TypeAssert, TypeSwitchStmt,
};
use super::super::position::Pos;
use super::super::token::Token;
use super::declarations::{gen_decl};
use super::expressions::{expr, expr_list, field_expr, optional_expr};
use super::{Converter, Missing, children, last_token, named, named_field, start, token};

const INITIALIZER: &str = "initializer";
const STATEMENT_LIST: &str = "statement_list";
const DEFAULT_CASE: &str = "default_case";
const FOR_CLAUSE: &str = "for_clause";
const RANGE_CLAUSE: &str = "range_clause";
const DEFINE: &str = ":=";
const SKIPPED_KINDS: [&str; 2] = ["comment", "\n"];

pub fn block(converter: &Converter<'_>, node: Node<'_>) -> Option<Block> {
    Some(Block {
        lbrace: token(node, "{"),
        list: statement_list(converter, node)?,
        rbrace: last_token(node, "}"),
    })
}

pub fn statement_list(converter: &Converter<'_>, parent: Node<'_>) -> Option<Vec<Stmt>> {
    let Some(list) = named(parent)
        .into_iter()
        .find(|child| child.kind() == STATEMENT_LIST)
    else {
        return Some(Vec::new());
    };
    named(list).into_iter().map(|child| stmt(converter, child)).collect()
}

pub fn stmt(converter: &Converter<'_>, node: Node<'_>) -> Option<Stmt> {
    match node.kind() {
        "expression_statement" => Some(Stmt::Expr(expr(converter, *named(node).first()?)?)),
        "send_statement" => Some(Stmt::Send(Box::new(SendStmt {
            chan: field_expr(converter, node, "channel")?,
            arrow: token(node, "<-"),
            value: field_expr(converter, node, "value")?,
        }))),
        "inc_statement" => inc_dec(converter, node, Token::Inc),
        "dec_statement" => inc_dec(converter, node, Token::Dec),
        "assignment_statement" => assignment(converter, node),
        "short_var_declaration" | "receive_statement" => definition(converter, node),
        "go_statement" => Some(Stmt::Go(Box::new(call_stmt(converter, node)?))),
        "defer_statement" => Some(Stmt::Defer(Box::new(call_stmt(converter, node)?))),
        "return_statement" => Some(Stmt::Return(Box::new(ReturnStmt {
            keyword: start(node),
            results: named(node)
                .first()
                .map_or_else(|| Some(Vec::new()), |list| expr_list(converter, *list))?,
        }))),
        "break_statement" => branch(converter, node, Token::Break),
        "continue_statement" => branch(converter, node, Token::Continue),
        "goto_statement" => branch(converter, node, Token::Goto),
        "fallthrough_statement" => branch(converter, node, Token::Fallthrough),
        "block" => Some(Stmt::Block(Box::new(block(converter, node)?))),
        "if_statement" => if_stmt(converter, node),
        "for_statement" => for_stmt(converter, node),
        "expression_switch_statement" => switch(converter, node),
        "type_switch_statement" => type_switch(converter, node),
        "select_statement" => select(converter, node),
        "labeled_statement" => labeled(converter, node),
        "empty_statement" => Some(Stmt::Empty(start(node))),
        _ => Some(Stmt::Decl(gen_decl(converter, node)?)),
    }
}

pub fn optional_stmt(converter: &Converter<'_>, node: Node<'_>, field: &str) -> Result<Option<Stmt>, Missing> {
    node.child_by_field_name(field)
        .map(|child| stmt(converter, child).ok_or(Missing))
        .transpose()
}

pub fn inc_dec(converter: &Converter<'_>, node: Node<'_>, tok: Token) -> Option<Stmt> {
    Some(Stmt::IncDec(Box::new(IncDecStmt {
        x: expr(converter, *named(node).first()?)?,
        tok_pos: token(node, tok.text()),
        tok,
    })))
}

pub fn assignment(converter: &Converter<'_>, node: Node<'_>) -> Option<Stmt> {
    let operator = node.child_by_field_name("operator")?;
    Some(Stmt::Assign(Box::new(AssignStmt {
        lhs: expr_list(converter, node.child_by_field_name("left")?)?,
        tok_pos: start(operator),
        tok: Token::from_text(converter.text(operator))?,
        rhs: expr_list(converter, node.child_by_field_name("right")?)?,
    })))
}

pub fn definition(converter: &Converter<'_>, node: Node<'_>) -> Option<Stmt> {
    let right = node.child_by_field_name("right")?;
    let Some(left) = node.child_by_field_name("left") else {
        return Some(Stmt::Expr(expr(converter, right)?));
    };
    let define = token(node, DEFINE);
    let (tok_pos, tok) = if define.is_valid() {
        (define, Token::Define)
    } else {
        (token(node, "="), Token::Assign)
    };
    let rhs = if right.kind() == "expression_list" {
        expr_list(converter, right)?
    } else {
        vec![expr(converter, right)?]
    };
    Some(Stmt::Assign(Box::new(AssignStmt {
        lhs: expr_list(converter, left)?,
        tok_pos,
        tok,
        rhs,
    })))
}

pub fn call_stmt(converter: &Converter<'_>, node: Node<'_>) -> Option<CallStmt> {
    Some(CallStmt {
        keyword: start(node),
        call: expr(converter, *named(node).first()?)?,
    })
}

pub fn branch(converter: &Converter<'_>, node: Node<'_>, tok: Token) -> Option<Stmt> {
    Some(Stmt::Branch(Box::new(BranchStmt {
        tok_pos: start(node),
        tok,
        label: named(node).first().map(|label| converter.ident(*label)),
    })))
}

pub fn if_stmt(converter: &Converter<'_>, node: Node<'_>) -> Option<Stmt> {
    Some(Stmt::If(Box::new(IfStmt {
        keyword: start(node),
        init: optional_stmt(converter, node, INITIALIZER).ok()?,
        cond: field_expr(converter, node, "condition")?,
        body: block(converter, node.child_by_field_name("consequence")?)?,
        alternative: optional_stmt(converter, node, "alternative").ok()?,
    })))
}

pub fn for_stmt(converter: &Converter<'_>, node: Node<'_>) -> Option<Stmt> {
    let body = block(converter, node.child_by_field_name("body")?)?;
    let header = named(node).into_iter().find(|child| child.kind() != "block");
    let mut statement = ForStmt {
        keyword: start(node),
        init: None,
        cond: None,
        post: None,
        body,
    };
    match header {
        Some(clause) if clause.kind() == RANGE_CLAUSE => return range(converter, node, clause, statement.body),
        Some(clause) if clause.kind() == FOR_CLAUSE => {
            statement.init = optional_stmt(converter, clause, INITIALIZER).ok()?;
            statement.cond = optional_expr(converter, clause, "condition").ok()?;
            statement.post = optional_stmt(converter, clause, "update").ok()?;
        }
        Some(condition) => statement.cond = Some(expr(converter, condition)?),
        None => {}
    }
    Some(Stmt::For(Box::new(statement)))
}

pub fn range(converter: &Converter<'_>, node: Node<'_>, clause: Node<'_>, body: Block) -> Option<Stmt> {
    let left = match clause.child_by_field_name("left") {
        Some(list) => expr_list(converter, list)?,
        None => Vec::new(),
    };
    let mut left = left.into_iter();
    let define = token(clause, DEFINE);
    let (tok_pos, tok) = if define.is_valid() {
        (define, Token::Define)
    } else {
        let assign = token(clause, "=");
        (
            assign,
            if assign.is_valid() {
                Token::Assign
            } else {
                Token::Illegal
            },
        )
    };
    Some(Stmt::Range(Box::new(RangeStmt {
        keyword: start(node),
        key: left.next(),
        value: left.next(),
        tok_pos,
        tok,
        x: field_expr(converter, clause, "right")?,
        body,
    })))
}

pub fn switch(converter: &Converter<'_>, node: Node<'_>) -> Option<Stmt> {
    Some(Stmt::Switch(Box::new(SwitchStmt {
        keyword: start(node),
        init: optional_stmt(converter, node, INITIALIZER).ok()?,
        tag: optional_expr(converter, node, "value").ok()?,
        body: clauses(converter, node)?,
    })))
}

pub fn clauses(converter: &Converter<'_>, node: Node<'_>) -> Option<Block> {
    let list = named(node)
        .into_iter()
        .filter(|child| child.start_byte() > token(node, "{").offset().unwrap_or_default())
        .map(|current| clause(converter, current))
        .collect::<Option<Vec<_>>>()?;
    Some(Block {
        lbrace: token(node, "{"),
        list,
        rbrace: last_token(node, "}"),
    })
}

pub fn clause(converter: &Converter<'_>, node: Node<'_>) -> Option<Stmt> {
    let keyword = start(node);
    let colon = token(node, ":");
    let body = statement_list(converter, node)?;
    match node.kind() {
        "communication_case" => Some(Stmt::Comm(Box::new(CommClause {
            keyword,
            comm: Some(stmt(converter, node.child_by_field_name("communication")?)?),
            colon,
            body,
        }))),
        "expression_case" => Some(case(
            keyword,
            Some(field_list_exprs(converter, node, "value")?),
            colon,
            body,
        )),
        "type_case" => Some(case(
            keyword,
            Some(field_list_exprs(converter, node, "type")?),
            colon,
            body,
        )),
        DEFAULT_CASE
            if node
                .parent()
                .is_some_and(|parent| parent.kind() == "select_statement") =>
        {
            Some(Stmt::Comm(Box::new(CommClause {
                keyword,
                comm: None,
                colon,
                body,
            })))
        }
        DEFAULT_CASE => Some(case(keyword, None, colon, body)),
        _ => None,
    }
}

pub fn field_list_exprs(converter: &Converter<'_>, node: Node<'_>, field: &str) -> Option<Vec<Expr>> {
    let nodes = named_field(node, field);
    if let [list] = nodes.as_slice()
        && list.kind() == "expression_list"
    {
        return expr_list(converter, *list);
    }
    nodes.into_iter().map(|child| expr(converter, child)).collect()
}

pub fn type_switch(converter: &Converter<'_>, node: Node<'_>) -> Option<Stmt> {
    let assert = Expr::TypeAssert(Box::new(TypeAssert {
        x: field_expr(converter, node, "value")?,
        lparen: token(node, "("),
        typ: None,
        rparen: token(node, ")"),
    }));
    let assign = match node.child_by_field_name("alias") {
        Some(alias) => Stmt::Assign(Box::new(AssignStmt {
            lhs: expr_list(converter, alias)?,
            tok_pos: token(node, DEFINE),
            tok: Token::Define,
            rhs: vec![assert],
        })),
        None => Stmt::Expr(assert),
    };
    Some(Stmt::TypeSwitch(Box::new(TypeSwitchStmt {
        keyword: start(node),
        init: optional_stmt(converter, node, INITIALIZER).ok()?,
        assign,
        body: clauses(converter, node)?,
    })))
}

pub fn select(converter: &Converter<'_>, node: Node<'_>) -> Option<Stmt> {
    Some(Stmt::Select(Box::new(SelectStmt {
        keyword: start(node),
        body: clauses(converter, node)?,
    })))
}

pub fn labeled(converter: &Converter<'_>, node: Node<'_>) -> Option<Stmt> {
    let label = node.child_by_field_name("label")?;
    let statement = named(node).into_iter().find(|child| *child != label);
    let stmt = match statement {
        Some(statement) => stmt(converter, statement)?,
        None => Stmt::Empty(following_token(node)),
    };
    Some(Stmt::Labeled(Box::new(LabeledStmt {
        label: converter.ident(label),
        colon: token(node, ":"),
        stmt,
    })))
}
fn case(keyword: Pos, list: Option<Vec<Expr>>, colon: Pos, body: Vec<Stmt>) -> Stmt {
    Stmt::Case(Box::new(CaseClause {
        keyword,
        list,
        colon,
        body,
    }))
}

fn following_token(node: Node<'_>) -> Pos {
    let mut current = node;
    loop {
        let mut sibling = current.next_sibling();
        while let Some(candidate) = sibling.filter(|candidate| SKIPPED_KINDS.contains(&candidate.kind())) {
            sibling = candidate.next_sibling();
        }
        if let Some(sibling) = sibling {
            return children(sibling)
                .first()
                .map_or_else(|| start(sibling), |first| start(*first));
        }
        let Some(parent) = current.parent() else {
            return Pos::NONE;
        };
        current = parent;
    }
}
