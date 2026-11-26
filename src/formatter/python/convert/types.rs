use tree_sitter::Node;

use super::expressions::{element_or_tuple, expression};
use super::{Converter, Outcome, has_token, named, only_named};
use crate::formatter::python::ast::{Expr, Operator};

const TYPE: &str = "type";
const STAR: &str = "*";

// tree-sitter lee una anotación con su propia gramática de tipos (`List[int]` es un `generic_type`,
// `int | str` un `union_type`); para ruff es una expresión cualquiera, y acá vuelve a serlo.
pub fn annotation<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree> {
    if node.kind() != TYPE {
        return expression(converter, node);
    }
    let inner = only_named(node)?;
    let added = match inner.kind() {
        "generic_type" => generic(converter, inner)?,
        "union_type" => union(converter, inner)?,
        "member_type" => member(converter, inner)?,
        "splat_type" if has_token(inner, STAR) => Expr::Starred {
            value: expression(converter, only_named(inner)?)?,
        },
        "constrained_type" => slice(converter, inner)?,
        "splat_type" => return Err(inner),
        _ => return expression(converter, inner),
    };
    Ok(converter.expression(inner, added))
}

fn generic<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree, Expr> {
    let children = named(node);
    let [name, parameters] = children.as_slice() else {
        return Err(node);
    };
    Ok(Expr::Subscript {
        value: converter.expression(*name, Expr::Name),
        slice: element_or_tuple(converter, *parameters, &named(*parameters))?,
    })
}

fn union<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree, Expr> {
    let children = named(node);
    let [left, right] = children.as_slice() else {
        return Err(node);
    };
    Ok(Expr::BinOp {
        left: annotation(converter, *left)?,
        op: Operator::BitOr,
        right: annotation(converter, *right)?,
    })
}

fn slice<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree, Expr> {
    let children = named(node);
    let [lower, upper] = children.as_slice() else {
        return Err(node);
    };
    Ok(Expr::Slice {
        lower: Some(annotation(converter, *lower)?),
        upper: Some(annotation(converter, *upper)?),
        step: None,
    })
}

fn member<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree, Expr> {
    let children = named(node);
    let [object, attribute] = children.as_slice() else {
        return Err(node);
    };
    Ok(Expr::Attribute {
        value: annotation(converter, *object)?,
        attr: super::span(*attribute),
    })
}
