use tree_sitter::Node;

use super::{Converter, Outcome, collections, field, has_token, named, only_named, parameters, span};
use super::{strings, types};
use crate::formatter::python::ast::{
    BoolOperator, CmpOperator, Data, Expr, NodeId, NumberKind, Operator, Span, UnaryOperator,
};

const COMMA: &str = ",";
const FROM: &str = "from";
const AND: &str = "and";
const IMAGINARY_SUFFIXES: [char; 2] = ['j', 'J'];
const LONG_SUFFIXES: [char; 2] = ['l', 'L'];
const BOOLEAN_OPERATOR: &str = "boolean_operator";

pub fn expression<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree> {
    let added = match node.kind() {
        "identifier" => Expr::Name,
        "integer" | "float" => number(converter, node)?,
        "true" => Expr::BooleanLiteral(true),
        "false" => Expr::BooleanLiteral(false),
        "none" => Expr::NoneLiteral,
        "ellipsis" => Expr::EllipsisLiteral,
        "parenthesized_expression" => return parenthesized(converter, node),
        "string" | "concatenated_string" => return strings::convert(converter, node),
        "type" => return types::annotation(converter, node),
        "attribute" => Expr::Attribute {
            value: expression(converter, field(node, "object")?)?,
            attr: span(field(node, "attribute")?),
        },
        "list_splat" | "list_splat_pattern" => Expr::Starred {
            value: expression(converter, only_named(node)?)?,
        },
        "await" => Expr::Await {
            value: expression(converter, only_named(node)?)?,
        },
        "yield" => yielded(converter, node)?,
        _ => return operation(converter, node),
    };
    Ok(converter.expression(node, added))
}

// Los paréntesis no son un nodo: queda anotado el tramo de lo que encierran, que es como ruff
// sabe después que la expresión estaba entre paréntesis.
pub fn parenthesized<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree> {
    let inner = expression(converter, only_named(node)?)?;
    let inner_span = converter.ast.span(inner);
    converter.parenthesized.insert(inner_span);
    Ok(inner)
}

fn operation<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree> {
    let added = match node.kind() {
        "binary_operator" => binary(converter, node)?,
        "unary_operator" => unary(converter, node)?,
        "not_operator" => Expr::UnaryOp {
            op: UnaryOperator::Not,
            operand: expression(converter, field(node, "argument")?)?,
        },
        BOOLEAN_OPERATOR => boolean(converter, node)?,
        "comparison_operator" => comparison(converter, node)?,
        "conditional_expression" => conditional(converter, node)?,
        "named_expression" => Expr::Named {
            target: expression(converter, field(node, "name")?)?,
            value: expression(converter, field(node, "value")?)?,
        },
        "lambda" => lambda(converter, node)?,
        _ => return collections::convert(converter, node),
    };
    Ok(converter.expression(node, added))
}

// Donde la gramática deja varias expresiones separadas por comas sin paréntesis, ruff arma una
// tupla sin paréntesis.
pub fn expression_or_tuple<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree> {
    match node.kind() {
        "expression_list" | "pattern_list" => tuple_of(converter, span(node), &named(node)),
        _ => expression(converter, node),
    }
}

pub fn tuple_of<'tree>(
    converter: &mut Converter<'_>,
    tuple_span: Span,
    elements: &[Node<'tree>],
) -> Outcome<'tree> {
    let elts = elements
        .iter()
        .map(|element| expression(converter, *element))
        .collect::<Outcome<'tree, Vec<NodeId>>>()?;
    let tuple = Expr::Tuple {
        elts,
        parenthesized: false,
    };
    Ok(converter.ast.add(tuple_span, Data::Expr(tuple)))
}

// `1, 2` sin paréntesis es una tupla; un solo elemento sin coma es el elemento.
pub fn element_or_tuple<'tree>(
    converter: &mut Converter<'_>,
    container: Node<'tree>,
    elements: &[Node<'tree>],
) -> Outcome<'tree> {
    let (Some(first), Some(last)) = (elements.first(), elements.last()) else {
        return Err(container);
    };
    if elements.len() == 1 && !has_token(container, COMMA) {
        return expression(converter, *first);
    }
    let end = last_comma_end(container, *last).unwrap_or_else(|| last.end_byte());
    tuple_of(converter, Span::new(first.start_byte(), end), elements)
}

fn last_comma_end(container: Node<'_>, last: Node<'_>) -> Option<usize> {
    super::parts(container)
        .into_iter()
        .find(|part| part.kind() == COMMA && part.start_byte() >= last.end_byte())
        .map(|comma| comma.end_byte())
}

fn number<'tree>(converter: &Converter<'_>, node: Node<'tree>) -> Outcome<'tree, Expr> {
    let text = converter.text(node);
    if text.ends_with(LONG_SUFFIXES) {
        return Err(node);
    }
    let kind = if text.ends_with(IMAGINARY_SUFFIXES) {
        NumberKind::Complex
    } else if node.kind() == "float" {
        NumberKind::Float
    } else {
        NumberKind::Int
    };
    Ok(Expr::NumberLiteral(kind))
}

fn yielded<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree, Expr> {
    let value = match named(node).as_slice() {
        [] => None,
        [value] => Some(expression_or_tuple(converter, *value)?),
        _ => return Err(node),
    };
    if has_token(node, FROM) {
        return Ok(Expr::YieldFrom {
            value: value.ok_or(node)?,
        });
    }
    Ok(Expr::Yield { value })
}

fn binary<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree, Expr> {
    let operator = field(node, "operator")?;
    Ok(Expr::BinOp {
        left: expression(converter, field(node, "left")?)?,
        op: Operator::from_token(converter.text(operator)).ok_or(operator)?,
        right: expression(converter, field(node, "right")?)?,
    })
}

fn unary<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree, Expr> {
    let operator = field(node, "operator")?;
    let op = match converter.text(operator) {
        "+" => UnaryOperator::UAdd,
        "-" => UnaryOperator::USub,
        "~" => UnaryOperator::Invert,
        _ => return Err(operator),
    };
    Ok(Expr::UnaryOp {
        op,
        operand: expression(converter, field(node, "argument")?)?,
    })
}

fn boolean_operator<'tree>(converter: &Converter<'_>, node: Node<'tree>) -> Outcome<'tree, BoolOperator> {
    let operator = field(node, "operator")?;
    Ok(if converter.text(operator) == AND {
        BoolOperator::And
    } else {
        BoolOperator::Or
    })
}

// `a and b and c` es un solo `BoolOp` de tres valores: tree-sitter lo anida a la izquierda y ruff
// lo aplana, salvo que el operando izquierdo esté entre paréntesis o sea de otro operador.
fn boolean<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree, Expr> {
    let op = boolean_operator(converter, node)?;
    let mut operands = vec![field(node, "right")?];
    let mut left = field(node, "left")?;
    while left.kind() == BOOLEAN_OPERATOR && boolean_operator(converter, left)? == op {
        operands.push(field(left, "right")?);
        left = field(left, "left")?;
    }
    operands.push(left);
    let values = operands
        .into_iter()
        .rev()
        .map(|operand| expression(converter, operand))
        .collect::<Outcome<'tree, Vec<NodeId>>>()?;
    Ok(Expr::BoolOp { op, values })
}

fn comparison<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree, Expr> {
    let mut cursor = node.walk();
    let ops = node
        .children_by_field_name("operators", &mut cursor)
        .map(|operator| CmpOperator::from_token(converter.text(operator)).ok_or(operator))
        .collect::<Outcome<'tree, Vec<CmpOperator>>>()?;
    let operands = named(node);
    let (left, comparators) = operands.split_first().ok_or(node)?;
    let left = expression(converter, *left)?;
    let comparators = comparators
        .iter()
        .map(|comparator| expression(converter, *comparator))
        .collect::<Outcome<'tree, Vec<NodeId>>>()?;
    if ops.len() != comparators.len() {
        return Err(node);
    }
    Ok(Expr::Compare {
        left,
        ops,
        comparators,
    })
}

fn conditional<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree, Expr> {
    let children = named(node);
    let [body, test, orelse] = children.as_slice() else {
        return Err(node);
    };
    Ok(Expr::IfExp {
        body: expression(converter, *body)?,
        test: expression(converter, *test)?,
        orelse: expression(converter, *orelse)?,
    })
}

fn lambda<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree, Expr> {
    let lambda_parameters = node
        .child_by_field_name("parameters")
        .map(|lambda_parameters| parameters::convert(converter, lambda_parameters))
        .transpose()?;
    Ok(Expr::Lambda {
        parameters: lambda_parameters,
        body: expression(converter, field(node, "body")?)?,
    })
}
