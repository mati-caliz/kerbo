use tree_sitter::Node;

use super::{Converter, Outcome, field, named, only_named, span, types};
use crate::formatter::python::ast::{Data, Expr, NodeId, Other, Span, Stmt};

const TYPE: &str = "type";
const IDENTIFIER: &str = "identifier";
const GENERIC_TYPE: &str = "generic_type";
const CONSTRAINED_TYPE: &str = "constrained_type";
const SPLAT_TYPE: &str = "splat_type";
const DEFAULT: &str = "type_parameter_default";
const TYPE_PARAMETERS_FIELD: &str = "type_parameters";
const PARAM_SPEC_PREFIX: &str = "**";

pub fn optional<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree, Option<NodeId>> {
    node.child_by_field_name(TYPE_PARAMETERS_FIELD)
        .map(|parameters| type_params(converter, parameters))
        .transpose()
}

fn add_other(converter: &mut Converter<'_>, bounds: Span, other: Other) -> NodeId {
    converter.ast.add(bounds, Data::Other(other))
}

fn type_params<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree> {
    let children = named(node);
    let mut params = Vec::new();
    let mut remaining = children.iter().peekable();
    while let Some(child) = remaining.next() {
        if child.kind() != TYPE {
            return Err(*child);
        }
        let default = match remaining.next_if(|next| next.kind() == DEFAULT) {
            Some(default) => Some((
                types::annotation(converter, only_named(*default)?)?,
                default.end_byte(),
            )),
            None => None,
        };
        params.push(param(converter, *child, default)?);
    }
    Ok(converter.other(node, Other::TypeParams { params }))
}

fn param<'tree>(
    converter: &mut Converter<'_>,
    child: Node<'tree>,
    default: Option<(NodeId, usize)>,
) -> Outcome<'tree> {
    let inner = only_named(child)?;
    let end = default.map_or_else(|| child.end_byte(), |(_, end)| end);
    let bounds = Span::new(child.start_byte(), end);
    let default_node = default.map(|(node, _)| node);
    let other = match inner.kind() {
        IDENTIFIER => Other::TypeVar {
            name: span(inner),
            bound: None,
            default: default_node,
        },
        CONSTRAINED_TYPE => constrained(converter, inner, default_node)?,
        SPLAT_TYPE => splat(converter, inner, default_node)?,
        _ => return Err(inner),
    };
    Ok(add_other(converter, bounds, other))
}

fn type_name(node: Node<'_>) -> Outcome<'_, Span> {
    let inner = if node.kind() == TYPE {
        only_named(node)?
    } else {
        node
    };
    if inner.kind() == IDENTIFIER {
        Ok(span(inner))
    } else {
        Err(inner)
    }
}

fn constrained<'tree>(
    converter: &mut Converter<'_>,
    node: Node<'tree>,
    default: Option<NodeId>,
) -> Outcome<'tree, Other> {
    let [name, bound] = named(node).as_slice().try_into().map_err(|_mismatch| node)?;
    Ok(Other::TypeVar {
        name: type_name(name)?,
        bound: Some(types::annotation(converter, bound)?),
        default,
    })
}

fn splat<'tree>(
    converter: &Converter<'_>,
    node: Node<'tree>,
    default: Option<NodeId>,
) -> Outcome<'tree, Other> {
    let name = span(only_named(node)?);
    let is_param_spec = converter.text(node).starts_with(PARAM_SPEC_PREFIX);
    Ok(if is_param_spec {
        Other::ParamSpec { name, default }
    } else {
        Other::TypeVarTuple { name, default }
    })
}

pub fn alias<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree, Stmt> {
    let left = field(node, "left")?;
    let inner = if left.kind() == TYPE {
        only_named(left)?
    } else {
        left
    };
    let (name_node, params) = if inner.kind() == GENERIC_TYPE {
        let [name, parameters] = named(inner).as_slice().try_into().map_err(|_mismatch| inner)?;
        (name, Some(type_params(converter, parameters)?))
    } else {
        (inner, None)
    };
    if name_node.kind() != IDENTIFIER {
        return Err(name_node);
    }
    let name = converter.expression(name_node, Expr::Name);
    let value = types::annotation(converter, field(node, "right")?)?;
    Ok(Stmt::TypeAlias {
        name,
        type_params: params,
        value,
    })
}
