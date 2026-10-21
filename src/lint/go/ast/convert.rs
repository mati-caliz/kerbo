use tree_sitter::Node;

use super::{Expr, File, Ident, IdentId, UnaryOp, declarations};

const COMMENT: &str = "comment";
const IDENTIFIER_KINDS: [&str; 10] = [
    "identifier",
    "type_identifier",
    "package_identifier",
    "field_identifier",
    "label_name",
    "blank_identifier",
    "true",
    "false",
    "nil",
    "iota",
];
const LITERAL_KINDS: [&str; 6] = [
    "int_literal",
    "float_literal",
    "imaginary_literal",
    "rune_literal",
    "interpreted_string_literal",
    "raw_string_literal",
];
const PANICKING_OPERATORS: [&str; 3] = ["==", "/", "%"];
const UNION_OPERATOR_PANICS: bool = false;

#[derive(Debug)]
pub struct Converter<'source> {
    pub source: &'source [u8],
    pub idents: Vec<Ident>,
}

pub fn file(root: Node<'_>, source: &[u8]) -> File {
    let mut converter = Converter {
        source,
        idents: Vec::new(),
    };
    let decls = children(root)
        .into_iter()
        .filter_map(|child| declarations::top_level(&mut converter, child))
        .collect();
    File {
        idents: converter.idents,
        decls,
    }
}

pub fn children(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .filter(|child| child.kind() != COMMENT)
        .collect()
}

pub fn field_nodes<'tree>(node: Node<'tree>, field: &str) -> Vec<Node<'tree>> {
    let mut cursor = node.walk();
    node.children_by_field_name(field, &mut cursor)
        .filter(|child| child.kind() != COMMENT)
        .collect()
}

impl Converter<'_> {
    pub fn text(&self, node: Node<'_>) -> &str {
        node.utf8_text(self.source).unwrap_or_default()
    }

    pub fn ident(&mut self, node: Node<'_>) -> IdentId {
        self.idents.push(Ident {
            name: self.text(node).to_owned(),
            offset: node.start_byte(),
            start: node.start_position(),
        });
        self.idents.len().saturating_sub(1)
    }

    pub fn field_expr(&mut self, node: Node<'_>, field: &str) -> Option<Expr> {
        node.child_by_field_name(field).map(|child| self.expr(child))
    }

    pub fn boxed_field(&mut self, node: Node<'_>, field: &str) -> Box<Expr> {
        Box::new(self.field_expr(node, field).unwrap_or_else(missing))
    }

    pub fn exprs(&mut self, nodes: Vec<Node<'_>>) -> Vec<Expr> {
        nodes.into_iter().map(|node| self.expr(node)).collect()
    }

    pub fn list(&mut self, node: Option<Node<'_>>) -> Vec<Expr> {
        node.map(|list| self.exprs(children(list))).unwrap_or_default()
    }

    pub fn expr(&mut self, node: Node<'_>) -> Expr {
        let kind = node.kind();
        if IDENTIFIER_KINDS.contains(&kind) {
            return Expr::Ident(self.ident(node));
        }
        if LITERAL_KINDS.contains(&kind) {
            return Expr::BasicLit(self.text(node).to_owned());
        }
        self.operation(node)
            .or_else(|| self.literal(node))
            .or_else(|| declarations::type_expr(self, node))
            .unwrap_or_else(missing)
    }

    fn operation(&mut self, node: Node<'_>) -> Option<Expr> {
        Some(match node.kind() {
            "parenthesized_expression" | "parenthesized_type" => Expr::Paren(Box::new(self.first(node))),
            "call_expression" => self.call(node),
            "type_conversion_expression" => Expr::Call {
                function: self.boxed_field(node, "type"),
                arguments: self.field_expr(node, "operand").into_iter().collect(),
            },
            "type_instantiation_expression" => Expr::Index {
                operand: self.boxed_field(node, "type"),
                indices: self.exprs(children(node).into_iter().skip(1).collect()),
            },
            "selector_expression" => Expr::Selector {
                operand: self.boxed_field(node, "operand"),
                field: self.field_ident(node, "field"),
            },
            "index_expression" => Expr::Index {
                operand: self.boxed_field(node, "operand"),
                indices: self.field_expr(node, "index").into_iter().collect(),
            },
            "slice_expression" => Expr::Slice {
                operand: self.boxed_field(node, "operand"),
                bounds: ["start", "end", "capacity"]
                    .into_iter()
                    .filter_map(|field| self.field_expr(node, field))
                    .collect(),
            },
            "type_assertion_expression" => Expr::TypeAssert {
                operand: self.boxed_field(node, "operand"),
                typ: Some(self.boxed_field(node, "type")),
            },
            "unary_expression" => self.unary(node),
            "binary_expression" => Expr::Binary {
                may_panic: node
                    .child_by_field_name("operator")
                    .is_some_and(|operator| PANICKING_OPERATORS.contains(&self.text(operator))),
                left: self.boxed_field(node, "left"),
                right: self.boxed_field(node, "right"),
            },
            "variadic_argument" => self.first(node),
            _ => return None,
        })
    }

    fn literal(&mut self, node: Node<'_>) -> Option<Expr> {
        Some(match node.kind() {
            "composite_literal" => Expr::CompositeLit {
                typ: Some(self.boxed_field(node, "type")),
                elements: node
                    .child_by_field_name("body")
                    .map(|body| self.exprs(children(body)))
                    .unwrap_or_default(),
            },
            "literal_value" => Expr::CompositeLit {
                typ: None,
                elements: self.exprs(children(node)),
            },
            "literal_element" => self.first(node),
            "keyed_element" => Expr::KeyValue {
                key: self.boxed_field(node, "key"),
                value: self.boxed_field(node, "value"),
            },
            "func_literal" => Expr::FuncLit {
                typ: declarations::func_type(self, node),
                body: declarations::body(self, node),
            },
            _ => return None,
        })
    }

    fn call(&mut self, node: Node<'_>) -> Expr {
        let mut function = self.boxed_field(node, "function");
        if let Some(arguments) = node.child_by_field_name("type_arguments") {
            function = Box::new(Expr::Index {
                operand: function,
                indices: self.exprs(children(arguments)),
            });
        }
        Expr::Call {
            function,
            arguments: self.list(node.child_by_field_name("arguments")),
        }
    }

    fn unary(&mut self, node: Node<'_>) -> Expr {
        let operand = self.boxed_field(node, "operand");
        let operator = node
            .child_by_field_name("operator")
            .map(|operator| self.text(operator))
            .unwrap_or_default();
        let op = match operator {
            "*" => return Expr::Star(operand),
            "&" => UnaryOp::Address,
            "<-" => UnaryOp::Receive,
            _ => UnaryOp::Other,
        };
        Expr::Unary { op, operand }
    }

    pub fn union(&mut self, node: Node<'_>) -> Expr {
        let mut terms = self.exprs(children(node)).into_iter();
        let first = terms.next().unwrap_or_else(missing);
        terms.fold(first, |left, right| Expr::Binary {
            may_panic: UNION_OPERATOR_PANICS,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn field_ident(&mut self, node: Node<'_>, field: &str) -> IdentId {
        match node.child_by_field_name(field) {
            Some(child) => self.ident(child),
            None => self.missing_ident(node),
        }
    }

    fn missing_ident(&mut self, node: Node<'_>) -> IdentId {
        self.idents.push(Ident {
            name: String::new(),
            offset: node.end_byte(),
            start: node.end_position(),
        });
        self.idents.len().saturating_sub(1)
    }

    pub fn first(&mut self, node: Node<'_>) -> Expr {
        children(node)
            .first()
            .map_or_else(missing, |child| self.expr(*child))
    }
}

pub const fn missing() -> Expr {
    Expr::BasicLit(String::new())
}

pub const fn unary_other(operand: Box<Expr>) -> Expr {
    Expr::Unary {
        op: UnaryOp::Other,
        operand,
    }
}
