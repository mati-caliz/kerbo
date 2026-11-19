use super::{BoolOperator, CmpOperator, NodeId, NumberKind, Operator, Span, UnaryOperator};

// Los nodos de `ruff_python_ast`, con los mismos nombres de campo. Un identificador no es un nodo:
// queda como el tramo del fuente que ocupa.
#[derive(Debug, Clone)]
pub enum Data {
    Stmt(Stmt),
    Expr(Expr),
    Other(Other),
}

impl Data {
    pub const fn is_statement(&self) -> bool {
        matches!(self, Self::Stmt(_))
    }

    pub const fn is_expression(&self) -> bool {
        matches!(self, Self::Expr(_))
    }
}

#[derive(Debug, Clone)]
pub enum Stmt {
    FunctionDef {
        is_async: bool,
        decorators: Vec<NodeId>,
        name: Span,
        type_params: Option<NodeId>,
        parameters: NodeId,
        returns: Option<NodeId>,
        body: Vec<NodeId>,
    },
    ClassDef {
        decorators: Vec<NodeId>,
        name: Span,
        type_params: Option<NodeId>,
        arguments: Option<NodeId>,
        body: Vec<NodeId>,
    },
    Return {
        value: Option<NodeId>,
    },
    Delete {
        targets: Vec<NodeId>,
    },
    Assign {
        targets: Vec<NodeId>,
        value: NodeId,
    },
    AugAssign {
        target: NodeId,
        op: Operator,
        value: NodeId,
    },
    AnnAssign {
        target: NodeId,
        annotation: NodeId,
        value: Option<NodeId>,
    },
    For {
        is_async: bool,
        target: NodeId,
        iter: NodeId,
        body: Vec<NodeId>,
        orelse: Vec<NodeId>,
    },
    While {
        test: NodeId,
        body: Vec<NodeId>,
        orelse: Vec<NodeId>,
    },
    If {
        test: NodeId,
        body: Vec<NodeId>,
        clauses: Vec<NodeId>,
    },
    With {
        is_async: bool,
        items: Vec<NodeId>,
        body: Vec<NodeId>,
    },
    Raise {
        exc: Option<NodeId>,
        cause: Option<NodeId>,
    },
    Try {
        body: Vec<NodeId>,
        handlers: Vec<NodeId>,
        orelse: Vec<NodeId>,
        finalbody: Vec<NodeId>,
        is_star: bool,
    },
    Assert {
        test: NodeId,
        msg: Option<NodeId>,
    },
    Import {
        names: Vec<NodeId>,
        is_lazy: bool,
    },
    ImportFrom {
        module: Option<Span>,
        names: Vec<NodeId>,
        level: usize,
        is_lazy: bool,
    },
    Global {
        names: Vec<Span>,
    },
    Nonlocal {
        names: Vec<Span>,
    },
    Expression {
        value: NodeId,
    },
    Pass,
    Break,
    Continue,
}

#[derive(Debug, Clone)]
pub enum Expr {
    BoolOp {
        op: BoolOperator,
        values: Vec<NodeId>,
    },
    Named {
        target: NodeId,
        value: NodeId,
    },
    BinOp {
        left: NodeId,
        op: Operator,
        right: NodeId,
    },
    UnaryOp {
        op: UnaryOperator,
        operand: NodeId,
    },
    Lambda {
        parameters: Option<NodeId>,
        body: NodeId,
    },
    IfExp {
        test: NodeId,
        body: NodeId,
        orelse: NodeId,
    },
    Dict {
        items: Vec<(Option<NodeId>, NodeId)>,
    },
    Set {
        elts: Vec<NodeId>,
    },
    ListComp {
        elt: NodeId,
        generators: Vec<NodeId>,
    },
    SetComp {
        elt: NodeId,
        generators: Vec<NodeId>,
    },
    DictComp {
        key: NodeId,
        value: NodeId,
        generators: Vec<NodeId>,
    },
    Generator {
        elt: NodeId,
        generators: Vec<NodeId>,
        parenthesized: bool,
    },
    Await {
        value: NodeId,
    },
    Yield {
        value: Option<NodeId>,
    },
    YieldFrom {
        value: NodeId,
    },
    Compare {
        left: NodeId,
        ops: Vec<CmpOperator>,
        comparators: Vec<NodeId>,
    },
    Call {
        func: NodeId,
        arguments: NodeId,
    },
    StringLiteral {
        parts: Vec<NodeId>,
    },
    BytesLiteral {
        parts: Vec<NodeId>,
    },
    NumberLiteral(NumberKind),
    BooleanLiteral(bool),
    NoneLiteral,
    EllipsisLiteral,
    Attribute {
        value: NodeId,
        attr: Span,
    },
    Subscript {
        value: NodeId,
        slice: NodeId,
    },
    Starred {
        value: NodeId,
    },
    Name,
    List {
        elts: Vec<NodeId>,
    },
    Tuple {
        elts: Vec<NodeId>,
        parenthesized: bool,
    },
    Slice {
        lower: Option<NodeId>,
        upper: Option<NodeId>,
        step: Option<NodeId>,
    },
}

#[derive(Debug, Clone)]
pub enum Other {
    Module {
        body: Vec<NodeId>,
    },
    ExceptHandler {
        type_: Option<NodeId>,
        name: Option<Span>,
        body: Vec<NodeId>,
    },
    ElifElseClause {
        test: Option<NodeId>,
        body: Vec<NodeId>,
    },
    Comprehension {
        is_async: bool,
        target: NodeId,
        iter: NodeId,
        ifs: Vec<NodeId>,
    },
    Arguments {
        args: Vec<NodeId>,
        keywords: Vec<NodeId>,
    },
    Keyword {
        arg: Option<Span>,
        value: NodeId,
    },
    Parameters {
        posonlyargs: Vec<NodeId>,
        args: Vec<NodeId>,
        vararg: Option<NodeId>,
        kwonlyargs: Vec<NodeId>,
        kwarg: Option<NodeId>,
    },
    Parameter {
        name: Span,
        annotation: Option<NodeId>,
    },
    ParameterWithDefault {
        parameter: NodeId,
        default: Option<NodeId>,
    },
    Alias {
        name: Span,
        asname: Option<Span>,
    },
    WithItem {
        context_expr: NodeId,
        optional_vars: Option<NodeId>,
    },
    Decorator {
        expression: NodeId,
    },
    StringPart,
    BytesPart,
}
