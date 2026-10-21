use tree_sitter::Node;

use super::convert::{Converter, children, field_nodes, missing};
use super::{AssignOp, BranchKind, Expr, ForStmt, IfStmt, RangeStmt, Stmt, declarations};

const DEFINE: &str = ":=";
const PLAIN_ASSIGN: &str = "=";
const DIVIDING_ASSIGNMENTS: [&str; 2] = ["/=", "%="];
const CASE_KINDS: [&str; 4] = [
    "expression_case",
    "type_case",
    "communication_case",
    "default_case",
];

pub fn block(converter: &mut Converter<'_>, node: Node<'_>) -> Vec<Stmt> {
    children(node)
        .into_iter()
        .flat_map(|child| {
            if child.kind() == "statement_list" {
                children(child)
            } else {
                vec![child]
            }
        })
        .filter_map(|statement| stmt(converter, statement))
        .collect()
}

pub fn stmt(converter: &mut Converter<'_>, node: Node<'_>) -> Option<Stmt> {
    if let Some(decl) = declarations::gen_decl(converter, node) {
        return Some(Stmt::Decl(decl));
    }
    simple(converter, node)
        .or_else(|| jump(converter, node))
        .or_else(|| compound(converter, node))
}

fn simple(converter: &mut Converter<'_>, node: Node<'_>) -> Option<Stmt> {
    Some(match node.kind() {
        "expression_statement" => Stmt::Expr(converter.first(node)),
        "send_statement" => Stmt::Send {
            channel: converter.field_expr(node, "channel").unwrap_or_else(missing),
            value: converter.field_expr(node, "value").unwrap_or_else(missing),
        },
        "inc_statement" | "dec_statement" => Stmt::IncDec(converter.first(node)),
        "assignment_statement" => {
            let operator = node
                .child_by_field_name("operator")
                .map(|operator| converter.text(operator).to_owned())
                .unwrap_or_default();
            assign(converter, node, assign_op(&operator))
        }
        "short_var_declaration" => assign(converter, node, AssignOp::Define),
        "empty_statement" => Stmt::Empty,
        _ => return None,
    })
}

fn jump(converter: &mut Converter<'_>, node: Node<'_>) -> Option<Stmt> {
    let kind = match node.kind() {
        "return_statement" => {
            return Some(Stmt::Return(converter.list(children(node).first().copied())));
        }
        "go_statement" => return Some(Stmt::Go(converter.first(node))),
        "defer_statement" => return Some(Stmt::Defer(converter.first(node))),
        "labeled_statement" => return Some(labeled(converter, node)),
        "break_statement" => BranchKind::Break,
        "continue_statement" => BranchKind::Continue,
        "goto_statement" => BranchKind::Goto,
        "fallthrough_statement" => BranchKind::Fallthrough,
        _ => return None,
    };
    let label = children(node).first().map(|label| converter.ident(*label));
    Some(Stmt::Branch { kind, label })
}

fn compound(converter: &mut Converter<'_>, node: Node<'_>) -> Option<Stmt> {
    Some(match node.kind() {
        "block" => Stmt::Block(block(converter, node)),
        "if_statement" => if_stmt(converter, node),
        "for_statement" => for_stmt(converter, node),
        "expression_switch_statement" => Stmt::Switch {
            init: initializer(converter, node),
            tag: converter.field_expr(node, "value"),
            body: cases(converter, node),
        },
        "type_switch_statement" => type_switch(converter, node),
        "select_statement" => Stmt::Select(cases(converter, node)),
        _ => return None,
    })
}

fn assign(converter: &mut Converter<'_>, node: Node<'_>, op: AssignOp) -> Stmt {
    Stmt::Assign {
        left: converter.list(node.child_by_field_name("left")),
        op,
        right: converter.list(node.child_by_field_name("right")),
    }
}

fn labeled(converter: &mut Converter<'_>, node: Node<'_>) -> Stmt {
    let label = converter.field_ident(node, "label");
    let statement = children(node)
        .into_iter()
        .skip(1)
        .find_map(|child| stmt(converter, child))
        .unwrap_or(Stmt::Empty);
    Stmt::Labeled {
        label,
        statement: Box::new(statement),
    }
}

fn initializer(converter: &mut Converter<'_>, node: Node<'_>) -> Option<Box<Stmt>> {
    node.child_by_field_name("initializer")
        .and_then(|init| stmt(converter, init))
        .map(Box::new)
}

fn if_stmt(converter: &mut Converter<'_>, node: Node<'_>) -> Stmt {
    let init = initializer(converter, node).map(|init| *init);
    let condition = converter.field_expr(node, "condition").unwrap_or_else(missing);
    let body = node
        .child_by_field_name("consequence")
        .map(|consequence| block(converter, consequence))
        .unwrap_or_default();
    let alternative = node
        .child_by_field_name("alternative")
        .and_then(|alternative| stmt(converter, alternative));
    Stmt::If(Box::new(IfStmt {
        init,
        condition,
        body,
        alternative,
    }))
}

fn for_stmt(converter: &mut Converter<'_>, node: Node<'_>) -> Stmt {
    let body = declarations::body(converter, node);
    let header = children(node).into_iter().find(|child| child.kind() != "block");
    match header {
        Some(clause) if clause.kind() == "range_clause" => range(converter, clause, body),
        Some(clause) if clause.kind() == "for_clause" => Stmt::For(Box::new(ForStmt {
            init: initializer(converter, clause).map(|init| *init),
            condition: converter.field_expr(clause, "condition"),
            post: clause
                .child_by_field_name("update")
                .and_then(|update| stmt(converter, update)),
            body,
        })),
        condition => Stmt::For(Box::new(ForStmt {
            init: None,
            condition: condition.map(|condition| converter.expr(condition)),
            post: None,
            body,
        })),
    }
}

fn range(converter: &mut Converter<'_>, clause: Node<'_>, body: Vec<Stmt>) -> Stmt {
    let mut bounds = converter.list(clause.child_by_field_name("left")).into_iter();
    Stmt::Range(Box::new(RangeStmt {
        key: bounds.next(),
        value: bounds.next(),
        define: has_token(converter, clause, DEFINE),
        operand: converter.field_expr(clause, "right").unwrap_or_else(missing),
        body,
    }))
}

fn type_switch(converter: &mut Converter<'_>, node: Node<'_>) -> Stmt {
    let operand = Box::new(converter.field_expr(node, "value").unwrap_or_else(missing));
    let assertion = Expr::TypeAssert { operand, typ: None };
    let assign = match node.child_by_field_name("alias") {
        Some(alias) => Stmt::Assign {
            left: converter.exprs(children(alias)),
            op: AssignOp::Define,
            right: vec![assertion],
        },
        None => Stmt::Expr(assertion),
    };
    Stmt::TypeSwitch {
        init: initializer(converter, node),
        assign: Box::new(assign),
        body: cases(converter, node),
    }
}

fn cases(converter: &mut Converter<'_>, node: Node<'_>) -> Vec<Stmt> {
    children(node)
        .into_iter()
        .filter(|child| CASE_KINDS.contains(&child.kind()))
        .map(|clause| case(converter, clause))
        .collect()
}

fn case(converter: &mut Converter<'_>, node: Node<'_>) -> Stmt {
    let body = children(node)
        .into_iter()
        .filter(|child| child.kind() == "statement_list")
        .flat_map(|list| block(converter, list))
        .collect();
    match node.kind() {
        "communication_case" => Stmt::Comm {
            communication: node
                .child_by_field_name("communication")
                .map(|exchange| Box::new(communication(converter, exchange))),
            body,
        },
        "expression_case" => Stmt::Case {
            list: Some(converter.list(node.child_by_field_name("value"))),
            body,
        },
        "type_case" => Stmt::Case {
            list: Some(converter.exprs(field_nodes(node, "type"))),
            body,
        },
        _ if node
            .parent()
            .is_some_and(|parent| parent.kind() == "select_statement") =>
        {
            Stmt::Comm {
                communication: None,
                body,
            }
        }
        _ => Stmt::Case { list: None, body },
    }
}

fn communication(converter: &mut Converter<'_>, node: Node<'_>) -> Stmt {
    if node.kind() == "send_statement" {
        return simple(converter, node).unwrap_or(Stmt::Empty);
    }
    let right = converter.field_expr(node, "right").unwrap_or_else(missing);
    match node.child_by_field_name("left") {
        Some(left) => Stmt::Assign {
            left: converter.exprs(children(left)),
            op: if has_token(converter, node, DEFINE) {
                AssignOp::Define
            } else {
                AssignOp::Plain
            },
            right: vec![right],
        },
        None => Stmt::Expr(right),
    }
}

fn has_token(converter: &Converter<'_>, node: Node<'_>, token: &str) -> bool {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .any(|child| !child.is_named() && converter.text(child) == token)
}

fn assign_op(operator: &str) -> AssignOp {
    if operator == PLAIN_ASSIGN {
        AssignOp::Plain
    } else {
        AssignOp::Compound {
            divides: DIVIDING_ASSIGNMENTS.contains(&operator),
        }
    }
}
