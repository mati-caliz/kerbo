use tree_sitter::Node;

use super::super::ast::{
    ArrayType, ChanDir, ChanType, Ellipsis, Expr, Field, FieldList, FuncType, InterfaceType, MapType,
    Selector, Star, StructType, Unary,
};
use super::super::position::Pos;
use super::super::token::Token;
use super::expressions::{expr, field_expr, instantiate, optional_expr, union};
use super::{Converter, Missing, children, last_token, named, named_field, start, token};

const PARAMETER_LIST: &str = "parameter_list";
const VARIADIC_PARAMETER: &str = "variadic_parameter_declaration";
const METHOD_ELEMENT: &str = "method_elem";
const ELEMENT: &str = "element";
const ARROW: &str = "<-";

pub fn typ(converter: &Converter<'_>, node: Node<'_>) -> Option<Expr> {
    match node.kind() {
        "qualified_type" => Some(Expr::Selector(Box::new(Selector {
            x: field_expr(converter, node, "package")?,
            sel: converter.ident(node.child_by_field_name("name")?),
        }))),
        "generic_type" => {
            let arguments = node.child_by_field_name("type_arguments")?;
            instantiate(converter, field_expr(converter, node, "type")?, arguments, &named(arguments))
        }
        "pointer_type" => Some(Expr::Star(Box::new(Star {
            star: start(node),
            x: expr(converter, *named(node).first()?)?,
        }))),
        "struct_type" => Some(Expr::Struct(Box::new(StructType {
            keyword: start(node),
            fields: struct_fields(converter, *named(node).first()?)?,
        }))),
        "interface_type" => interface(converter, node),
        "array_type" => array(converter, node, optional_expr(converter, node, "length").ok()?),
        "implicit_length_array_type" => {
            let ellipsis = Expr::Ellipsis(Box::new(Ellipsis {
                pos: token(node, "..."),
                elt: None,
            }));
            array(converter, node, Some(ellipsis))
        }
        "slice_type" => array(converter, node, None),
        "map_type" => Some(Expr::Map(Box::new(MapType {
            keyword: start(node),
            key: field_expr(converter, node, "key")?,
            value: field_expr(converter, node, "value")?,
        }))),
        "channel_type" => channel(converter, node),
        "function_type" => Some(Expr::Func(Box::new(signature(converter, node, start(node), None)?))),
        "negated_type" => Some(Expr::Unary(Box::new(Unary {
            op_pos: start(node),
            op: Token::Tilde,
            x: expr(converter, *named(node).first()?)?,
        }))),
        "type_elem" | "type_constraint" => union(converter, node),
        _ => None,
    }
}

pub fn array(converter: &Converter<'_>, node: Node<'_>, len: Option<Expr>) -> Option<Expr> {
    Some(Expr::Array(Box::new(ArrayType {
        lbrack: start(node),
        len,
        elt: field_expr(converter, node, ELEMENT)?,
    })))
}

pub fn channel(converter: &Converter<'_>, node: Node<'_>) -> Option<Expr> {
    let tokens = children(node);
    let first = tokens.first()?;
    let arrow = token(node, ARROW);
    let dir = if first.kind() == ARROW {
        ChanDir::Receive
    } else if arrow.is_valid() {
        ChanDir::Send
    } else {
        ChanDir::Both
    };
    Some(Expr::Chan(Box::new(ChanType {
        begin: start(*first),
        arrow,
        dir,
        value: field_expr(converter, node, "value")?,
    })))
}

pub fn signature(converter: &Converter<'_>, node: Node<'_>, func: Pos, type_params: Option<FieldList>) -> Option<FuncType> {
    Some(FuncType {
        func,
        type_params,
        params: field_list(converter, node.child_by_field_name("parameters")?)?,
        results: results(converter, node).ok()?,
    })
}

pub fn results(converter: &Converter<'_>, node: Node<'_>) -> Result<Option<FieldList>, Missing> {
    let Some(result) = node.child_by_field_name("result") else {
        return Ok(None);
    };
    if result.kind() == PARAMETER_LIST {
        return field_list(converter, result).map(Some).ok_or(Missing);
    }
    let typ = expr(converter, result).ok_or(Missing)?;
    Ok(Some(FieldList {
        opening: Pos::NONE,
        list: vec![Field {
            names: Vec::new(),
            typ,
            tag: None,
            comment: None,
        }],
        closing: Pos::NONE,
    }))
}

pub fn field_list(converter: &Converter<'_>, node: Node<'_>) -> Option<FieldList> {
    let list = named(node)
        .into_iter()
        .map(|current| parameter(converter, current))
        .collect::<Option<Vec<_>>>()?;
    let (open, close) = if node.kind() == PARAMETER_LIST {
        ("(", ")")
    } else {
        ("[", "]")
    };
    Some(FieldList {
        opening: token(node, open),
        list,
        closing: last_token(node, close),
    })
}

pub fn parameter(converter: &Converter<'_>, node: Node<'_>) -> Option<Field> {
    let typ = node.child_by_field_name("type")?;
    let names = named_field(node, "name")
        .into_iter()
        .map(|name| converter.ident(name))
        .collect();
    let converted = expr(converter, typ)?;
    let typ = if node.kind() == VARIADIC_PARAMETER {
        Expr::Ellipsis(Box::new(Ellipsis {
            pos: token(node, "..."),
            elt: Some(converted),
        }))
    } else {
        converted
    };
    Some(Field {
        names,
        typ,
        tag: None,
        comment: None,
    })
}

pub fn struct_fields(converter: &Converter<'_>, node: Node<'_>) -> Option<FieldList> {
    let list = named(node)
        .into_iter()
        .map(|field| struct_field(converter, field))
        .collect::<Option<Vec<_>>>()?;
    Some(FieldList {
        opening: token(node, "{"),
        list,
        closing: last_token(node, "}"),
    })
}

pub fn struct_field(converter: &Converter<'_>, node: Node<'_>) -> Option<Field> {
    let names: Vec<_> = named_field(node, "name")
        .into_iter()
        .map(|name| converter.ident(name))
        .collect();
    let mut typ = field_expr(converter, node, "type")?;
    let star = token(node, "*");
    if names.is_empty() && star.is_valid() {
        typ = Expr::Star(Box::new(Star { star, x: typ }));
    }
    let tag = node
        .child_by_field_name("tag")
        .map(|tag| converter.literal(tag, Token::String));
    Some(Field {
        names,
        typ,
        tag,
        comment: converter.line_comment(node),
    })
}

pub fn interface(converter: &Converter<'_>, node: Node<'_>) -> Option<Expr> {
    let list = named(node)
        .into_iter()
        .map(|element| interface_element(converter, element))
        .collect::<Option<Vec<_>>>()?;
    Some(Expr::Interface(Box::new(InterfaceType {
        keyword: start(node),
        methods: FieldList {
            opening: token(node, "{"),
            list,
            closing: last_token(node, "}"),
        },
    })))
}

pub fn interface_element(converter: &Converter<'_>, node: Node<'_>) -> Option<Field> {
    if node.kind() == METHOD_ELEMENT {
        let signature = signature(converter, node, Pos::NONE, None)?;
        return Some(Field {
            names: vec![converter.ident(node.child_by_field_name("name")?)],
            typ: Expr::Func(Box::new(signature)),
            tag: None,
            comment: converter.line_comment(node),
        });
    }
    Some(Field {
        names: Vec::new(),
        typ: union(converter, node)?,
        tag: None,
        comment: converter.line_comment(node),
    })
}