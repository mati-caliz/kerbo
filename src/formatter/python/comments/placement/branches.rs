use crate::formatter::python::ast::{Ast, Data, NodeId, Other, Stmt};

fn is_first(node: NodeId, body: &[NodeId]) -> bool {
    body.first() == Some(&node)
}

pub fn is_first_statement_in_body(ast: &Ast, node: NodeId, owner: NodeId) -> bool {
    match ast.data(owner) {
        Data::Stmt(Stmt::For { body, orelse, .. } | Stmt::While { body, orelse, .. }) => {
            is_first(node, body) || is_first(node, orelse)
        }
        Data::Stmt(Stmt::Try {
            body,
            orelse,
            finalbody,
            ..
        }) => is_first(node, body) || is_first(node, orelse) || is_first(node, finalbody),
        Data::Stmt(
            Stmt::If { body, .. }
            | Stmt::With { body, .. }
            | Stmt::FunctionDef { body, .. }
            | Stmt::ClassDef { body, .. },
        )
        | Data::Other(Other::ElifElseClause { body, .. } | Other::ExceptHandler { body, .. }) => {
            is_first(node, body)
        }
        Data::Stmt(_) | Data::Expr(_) | Data::Other(_) => false,
    }
}

// Si `node` abre un `else`, `elif`, `except` o `finally` de `owner`.
pub fn is_first_statement_in_alternate_body(ast: &Ast, node: NodeId, owner: NodeId) -> bool {
    match ast.stmt(owner) {
        Some(Stmt::For { orelse, .. } | Stmt::While { orelse, .. }) => is_first(node, orelse),
        Some(Stmt::Try {
            handlers,
            orelse,
            finalbody,
            ..
        }) => is_first(node, handlers) || is_first(node, orelse) || is_first(node, finalbody),
        Some(Stmt::If { clauses, .. }) => is_first(node, clauses),
        Some(_) | None => false,
    }
}

// Un `elif`, `else` de un `if` o `except`: ramas que tienen nodo propio, a diferencia del `else`
// de un bucle o del `finally`.
pub fn is_alternative_branch_with_node(ast: &Ast, node: NodeId) -> bool {
    matches!(
        ast.other(node),
        Some(Other::ExceptHandler { .. } | Other::ElifElseClause { .. })
    )
}

fn last_branch_of_if<'ast>(ast: &'ast Ast, body: &'ast [NodeId], clauses: &[NodeId]) -> &'ast [NodeId] {
    match clauses.last().and_then(|clause| ast.other(*clause)) {
        Some(Other::ElifElseClause {
            body: clause_body, ..
        }) => clause_body,
        Some(_) | None => body,
    }
}

const fn last_of(body: &[NodeId]) -> Option<NodeId> {
    body.last().copied()
}

// La última sentencia de la última rama del nodo, si tiene cuerpo.
pub fn last_child_in_body(ast: &Ast, node: NodeId) -> Option<NodeId> {
    match ast.data(node) {
        Data::Stmt(
            Stmt::FunctionDef { body, .. } | Stmt::ClassDef { body, .. } | Stmt::With { body, .. },
        )
        | Data::Other(Other::ExceptHandler { body, .. } | Other::ElifElseClause { body, .. }) => {
            last_of(body)
        }
        Data::Stmt(Stmt::If { body, clauses, .. }) => last_of(last_branch_of_if(ast, body, clauses)),
        Data::Stmt(Stmt::For { body, orelse, .. } | Stmt::While { body, orelse, .. }) => {
            last_of(if orelse.is_empty() { body } else { orelse })
        }
        Data::Stmt(Stmt::Try {
            body,
            handlers,
            orelse,
            finalbody,
            ..
        }) => [finalbody, orelse, handlers, body]
            .into_iter()
            .find_map(|branch| last_of(branch)),
        Data::Stmt(_) | Data::Expr(_) | Data::Other(_) => None,
    }
}

pub fn innermost_last_child(ast: &Ast, node: NodeId) -> NodeId {
    std::iter::successors(Some(node), |current| last_child_in_body(ast, *current))
        .last()
        .unwrap_or(node)
}
