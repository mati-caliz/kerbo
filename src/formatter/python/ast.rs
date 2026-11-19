mod children;
mod data;
mod operators;

pub use data::{Data, Expr, Other, Stmt};
pub use operators::{BoolOperator, CmpOperator, NumberKind, Operator, UnaryOperator};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

#[derive(Debug, Clone)]
pub struct Node {
    pub span: Span,
    pub parent: Option<NodeId>,
    pub data: Data,
}

// El mismo árbol que arma el parser de ruff (`ruff_python_ast`), en un arreglo: cada nodo se
// nombra por su posición y conoce a su padre. Lo arma `convert` desde el árbol de tree-sitter.
#[derive(Debug, Default)]
pub struct Ast {
    nodes: Vec<Node>,
}

const PLACEHOLDER: Node = Node {
    span: Span::new(0, 0),
    parent: None,
    data: Data::Stmt(Stmt::Pass),
};

impl Ast {
    pub fn add(&mut self, span: Span, data: Data) -> NodeId {
        let id = NodeId(self.nodes.len());
        self.nodes.push(Node {
            span,
            parent: None,
            data,
        });
        id
    }

    pub fn node(&self, id: NodeId) -> &Node {
        self.nodes.get(id.0).unwrap_or(&PLACEHOLDER)
    }

    pub fn data(&self, id: NodeId) -> &Data {
        &self.node(id).data
    }

    pub fn expr(&self, id: NodeId) -> Option<&Expr> {
        match self.data(id) {
            Data::Expr(expression) => Some(expression),
            Data::Stmt(_) | Data::Other(_) => None,
        }
    }

    pub fn stmt(&self, id: NodeId) -> Option<&Stmt> {
        match self.data(id) {
            Data::Stmt(statement) => Some(statement),
            Data::Expr(_) | Data::Other(_) => None,
        }
    }

    pub fn other(&self, id: NodeId) -> Option<&Other> {
        match self.data(id) {
            Data::Other(other) => Some(other),
            Data::Stmt(_) | Data::Expr(_) => None,
        }
    }

    pub fn span(&self, id: NodeId) -> Span {
        self.node(id).span
    }

    pub fn start(&self, id: NodeId) -> usize {
        self.node(id).span.start
    }

    pub fn end(&self, id: NodeId) -> usize {
        self.node(id).span.end
    }

    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.node(id).parent
    }

    // Los hijos en el orden del fuente, como los recorre `SourceOrderVisitor` de ruff.
    pub fn children(&self, id: NodeId) -> Vec<NodeId> {
        let mut children = children::of(self.data(id));
        if matches!(self.data(id), Data::Other(Other::Arguments { .. })) {
            children.sort_by_key(|child| self.start(*child));
        }
        children
    }

    fn has_body(&self, id: NodeId) -> bool {
        matches!(
            self.data(id),
            Data::Stmt(
                Stmt::FunctionDef { .. }
                    | Stmt::ClassDef { .. }
                    | Stmt::For { .. }
                    | Stmt::While { .. }
                    | Stmt::If { .. }
                    | Stmt::With { .. }
                    | Stmt::Try { .. }
            ) | Data::Other(Other::ExceptHandler { .. } | Other::ElifElseClause { .. })
        )
    }

    // tree-sitter deja adentro de un bloque los comentarios que lo siguen; en ruff una sentencia
    // compuesta termina donde termina la última sentencia de su último cuerpo. Los hijos están
    // antes que sus padres en el arreglo, así que una pasada alcanza.
    pub fn end_bodies_at_their_last_statement(&mut self) {
        for index in 0..self.nodes.len() {
            let id = NodeId(index);
            if !self.has_body(id) {
                continue;
            }
            let Some(end) = self.children(id).last().map(|last| self.end(*last)) else {
                continue;
            };
            if let Some(node) = self.nodes.get_mut(index) {
                node.span.end = end;
            }
        }
    }

    fn set_parent(&mut self, child: NodeId, parent: NodeId) {
        if let Some(node) = self.nodes.get_mut(child.0) {
            node.parent = Some(parent);
        }
    }

    pub fn link_parents(&mut self, root: NodeId) {
        let mut pending = vec![root];
        while let Some(parent) = pending.pop() {
            for child in self.children(parent) {
                self.set_parent(child, parent);
                pending.push(child);
            }
        }
    }
}
