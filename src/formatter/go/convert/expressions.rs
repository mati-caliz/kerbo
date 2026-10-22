use tree_sitter::Node;

use super::super::ast::{
    Binary, Call, CompositeLit, Expr, FuncLit, FuncType, Index, IndexList, KeyValue, Paren, Selector, Slice,
    Star, TypeAssert, Unary,
};
use super::super::position::Pos;
use super::super::token::Token;
use super::statements::{block};
use super::types::{field_list, results, typ};
use super::{Converter, Missing, children, last_token, named, start, token};

const OPERATOR: &str = "operator";
const OPERAND: &str = "operand";
const DEREFERENCE: &str = "*";
const VARIADIC_ARGUMENT: &str = "variadic_argument";
const KEYED_ELEMENT: &str = "keyed_element";
const LITERAL_ELEMENT: &str = "literal_element";
const POINTER_TYPE: &str = "pointer_type";

pub fn expr(converter: &Converter<'_>, node: Node<'_>) -> Option<Expr> {
    let literal = |kind| Some(Expr::BasicLit(converter.literal(node, kind)));
    match node.kind() {
        "identifier" | "type_identifier" | "field_identifier" | "package_identifier" | "label_name"
        | "blank_identifier" | "dot" | "nil" | "true" | "false" | "iota" => {
            Some(Expr::Ident(converter.ident(node)))
        }
        "int_literal" => literal(Token::Int),
        "float_literal" => literal(Token::Float),
        "imaginary_literal" => literal(Token::Imag),
        "rune_literal" => literal(Token::Char),
        "interpreted_string_literal" | "raw_string_literal" => literal(Token::String),
        "parenthesized_expression" | "parenthesized_type" => paren(converter, node),
        "call_expression" => call(converter, node),
        "type_conversion_expression" => conversion(converter, node),
        "type_instantiation_expression" => instantiation(converter, node),
        "selector_expression" => selector(converter, node),
        "index_expression" => index(converter, node),
        "slice_expression" => slice(converter, node),
        "type_assertion_expression" => assertion(converter, node),
        "composite_literal" => composite(converter, node),
        "literal_value" => literal_value(converter, node, None),
        "func_literal" => func_literal(converter, node),
        "unary_expression" => unary(converter, node),
        "binary_expression" => binary(converter, node),
        _ => typ(converter, node),
    }
}

pub fn field_expr(converter: &Converter<'_>, node: Node<'_>, field: &str) -> Option<Expr> {
    expr(converter, node.child_by_field_name(field)?)
}

pub fn optional_expr(converter: &Converter<'_>, node: Node<'_>, field: &str) -> Result<Option<Expr>, Missing> {
    node.child_by_field_name(field)
        .map(|child| expr(converter, child).ok_or(Missing))
        .transpose()
}

pub fn expr_list(converter: &Converter<'_>, node: Node<'_>) -> Option<Vec<Expr>> {
    named(node).into_iter().map(|child| expr(converter, child)).collect()
}

pub fn paren(converter: &Converter<'_>, node: Node<'_>) -> Option<Expr> {
    Some(Expr::Paren(Box::new(Paren {
        lparen: start(node),
        x: expr(converter, *named(node).first()?)?,
        rparen: last_token(node, ")"),
    })))
}

pub fn call(converter: &Converter<'_>, node: Node<'_>) -> Option<Expr> {
    let mut fun = field_expr(converter, node, "function")?;
    if let Some(arguments) = node.child_by_field_name("type_arguments") {
        fun = instantiate(converter, fun, arguments, &named(arguments))?;
    }
    let arguments = node.child_by_field_name("arguments")?;
    let mut ellipsis = Pos::NONE;
    let mut args = Vec::new();
    for argument in named(arguments) {
        if argument.kind() == VARIADIC_ARGUMENT {
            ellipsis = token(argument, "...");
            args.push(expr(converter, *named(argument).first()?)?);
        } else {
            args.push(expr(converter, argument)?);
        }
    }
    Some(Expr::Call(Box::new(Call {
        fun,
        lparen: token(arguments, "("),
        args,
        ellipsis,
        rparen: last_token(arguments, ")"),
    })))
}

// tree-sitter lee `*T(x)` como una conversión al tipo `*T`; go/parser, como `*` aplicado a la
// conversión `T(x)`, que es lo que dice la especificación.
pub fn conversion(converter: &Converter<'_>, node: Node<'_>) -> Option<Expr> {
    let mut typ = node.child_by_field_name("type")?;
    let mut stars = Vec::new();
    while typ.kind() == POINTER_TYPE {
        stars.push(start(typ));
        typ = *named(typ).first()?;
    }
    let call = Expr::Call(Box::new(Call {
        fun: expr(converter, typ)?,
        lparen: token(node, "("),
        args: vec![field_expr(converter, node, OPERAND)?],
        ellipsis: Pos::NONE,
        rparen: last_token(node, ")"),
    }));
    Some(
        stars
            .into_iter()
            .rev()
            .fold(call, |x, star| Expr::Star(Box::new(Star { star, x }))),
    )
}

pub fn instantiation(converter: &Converter<'_>, node: Node<'_>) -> Option<Expr> {
    let typ = node.child_by_field_name("type")?;
    let indices: Vec<Node<'_>> = named(node).into_iter().filter(|child| *child != typ).collect();
    instantiate(converter, expr(converter, typ)?, node, &indices)
}

pub fn instantiate(converter: &Converter<'_>, x: Expr, brackets: Node<'_>, indices: &[Node<'_>]) -> Option<Expr> {
    let lbrack = token(brackets, "[");
    let rbrack = last_token(brackets, "]");
    let mut converted = indices
        .iter()
        .map(|index| expr(converter, *index))
        .collect::<Option<Vec<_>>>()?;
    if converted.len() == 1 {
        let index = converted.pop()?;
        return Some(Expr::Index(Box::new(Index {
            x,
            lbrack,
            index,
            rbrack,
        })));
    }
    Some(Expr::IndexList(Box::new(IndexList {
        x,
        lbrack,
        indices: converted,
        rbrack,
    })))
}

pub fn selector(converter: &Converter<'_>, node: Node<'_>) -> Option<Expr> {
    Some(Expr::Selector(Box::new(Selector {
        x: field_expr(converter, node, OPERAND)?,
        sel: converter.ident(node.child_by_field_name("field")?),
    })))
}

pub fn index(converter: &Converter<'_>, node: Node<'_>) -> Option<Expr> {
    Some(Expr::Index(Box::new(Index {
        x: field_expr(converter, node, OPERAND)?,
        lbrack: token(node, "["),
        index: field_expr(converter, node, "index")?,
        rbrack: last_token(node, "]"),
    })))
}

pub fn slice(converter: &Converter<'_>, node: Node<'_>) -> Option<Expr> {
    Some(Expr::Slice(Box::new(Slice {
        x: field_expr(converter, node, OPERAND)?,
        lbrack: token(node, "["),
        low: optional_expr(converter, node, "start").ok()?,
        high: optional_expr(converter, node, "end").ok()?,
        max: optional_expr(converter, node, "capacity").ok()?,
        rbrack: last_token(node, "]"),
    })))
}

pub fn assertion(converter: &Converter<'_>, node: Node<'_>) -> Option<Expr> {
    Some(Expr::TypeAssert(Box::new(TypeAssert {
        x: field_expr(converter, node, OPERAND)?,
        lparen: token(node, "("),
        typ: Some(field_expr(converter, node, "type")?),
        rparen: last_token(node, ")"),
    })))
}

pub fn composite(converter: &Converter<'_>, node: Node<'_>) -> Option<Expr> {
    let typ = field_expr(converter, node, "type")?;
    literal_value(converter, node.child_by_field_name("body")?, Some(typ))
}

pub fn literal_value(converter: &Converter<'_>, node: Node<'_>, typ: Option<Expr>) -> Option<Expr> {
    let elts = named(node)
        .into_iter()
        .map(|current| element(converter, current))
        .collect::<Option<Vec<_>>>()?;
    Some(Expr::Composite(Box::new(CompositeLit {
        typ,
        lbrace: token(node, "{"),
        elts,
        rbrace: last_token(node, "}"),
    })))
}

pub fn element(converter: &Converter<'_>, node: Node<'_>) -> Option<Expr> {
    match node.kind() {
        KEYED_ELEMENT => Some(Expr::KeyValue(Box::new(KeyValue {
            key: element(converter, node.child_by_field_name("key")?)?,
            colon: token(node, ":"),
            value: element(converter, node.child_by_field_name("value")?)?,
        }))),
        LITERAL_ELEMENT => element(converter, *named(node).first()?),
        _ => expr(converter, node),
    }
}

pub fn func_literal(converter: &Converter<'_>, node: Node<'_>) -> Option<Expr> {
    Some(Expr::FuncLit(Box::new(FuncLit {
        typ: FuncType {
            func: start(node),
            type_params: None,
            params: field_list(converter, node.child_by_field_name("parameters")?)?,
            results: results(converter, node).ok()?,
        },
        body: block(converter, node.child_by_field_name("body")?)?,
    })))
}

pub fn unary(converter: &Converter<'_>, node: Node<'_>) -> Option<Expr> {
    let operator = node.child_by_field_name(OPERATOR)?;
    let x = field_expr(converter, node, OPERAND)?;
    if converter.text(operator) == DEREFERENCE {
        return Some(Expr::Star(Box::new(Star {
            star: start(operator),
            x,
        })));
    }
    Some(Expr::Unary(Box::new(Unary {
        op_pos: start(operator),
        op: Token::from_text(converter.text(operator))?,
        x,
    })))
}

pub fn binary(converter: &Converter<'_>, node: Node<'_>) -> Option<Expr> {
    let operator = node.child_by_field_name(OPERATOR)?;
    Some(Expr::Binary(Box::new(Binary {
        x: field_expr(converter, node, "left")?,
        op_pos: start(operator),
        op: Token::from_text(converter.text(operator))?,
        y: field_expr(converter, node, "right")?,
    })))
}

pub fn union(converter: &Converter<'_>, node: Node<'_>) -> Option<Expr> {
    let mut operands = named(node).into_iter();
    let mut union = expr(converter, operands.next()?)?;
    let bars = children(node)
        .into_iter()
        .filter(|child| !child.is_named() && child.kind() == "|")
        .map(start);
    for (bar, operand) in bars.zip(operands) {
        union = Expr::Binary(Box::new(Binary {
            x: union,
            op_pos: bar,
            op: Token::Or,
            y: expr(converter, operand)?,
        }));
    }
    Some(union)
}