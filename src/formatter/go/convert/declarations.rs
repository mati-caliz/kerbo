use tree_sitter::Node;

use super::super::ast::{Decl, FieldList, FuncDecl, GenDecl, ImportSpec, Spec, TypeSpec, ValueSpec};
use super::super::position::Pos;
use super::super::token::Token;
use super::expressions::{expr_list, field_expr, optional_expr};
use super::statements::{block};
use super::types::{field_list, signature};
use super::{Converter, Missing, last_token, named, named_field, start, token};

const SPEC_LISTS: [&str; 2] = ["import_spec_list", "var_spec_list"];
const TYPE_PARAMETERS: &str = "type_parameters";

pub fn top_level(converter: &Converter<'_>, node: Node<'_>) -> Option<Decl> {
    match node.kind() {
        "function_declaration" => function(converter, node, None),
        "method_declaration" => {
            let receiver = field_list(converter, node.child_by_field_name("receiver")?)?;
            function(converter, node, Some(receiver))
        }
        "import_declaration" | "const_declaration" | "var_declaration" | "type_declaration" => {
            Some(Decl::Gen(gen_decl(converter, node)?))
        }
        _ => None,
    }
}

pub fn type_parameters(converter: &Converter<'_>, node: Node<'_>) -> Result<Option<FieldList>, Missing> {
    node.child_by_field_name(TYPE_PARAMETERS)
        .map(|parameters| field_list(converter, parameters).ok_or(Missing))
        .transpose()
}

pub fn function(converter: &Converter<'_>, node: Node<'_>, recv: Option<FieldList>) -> Option<Decl> {
    let type_params = type_parameters(converter, node).ok()?;
    let body = node
        .child_by_field_name("body")
        .map_or(Some(None), |body| block(converter, body).map(Some))?;
    Some(Decl::Func(Box::new(FuncDecl {
        recv,
        name: converter.ident(node.child_by_field_name("name")?),
        typ: signature(converter, node, start(node), type_params)?,
        body,
    })))
}

pub fn gen_decl(converter: &Converter<'_>, node: Node<'_>) -> Option<GenDecl> {
    let tok = match node.kind() {
        "import_declaration" => Token::Import,
        "const_declaration" => Token::Const,
        "var_declaration" => Token::Var,
        "type_declaration" => Token::Type,
        _ => return None,
    };
    let parenthesized = named(node)
        .into_iter()
        .find(|child| SPEC_LISTS.contains(&child.kind()))
        .unwrap_or(node);
    let specs = named(parenthesized)
        .into_iter()
        .map(|current| spec(converter, current))
        .collect::<Option<Vec<_>>>()?;
    Some(GenDecl {
        tok_pos: start(node),
        tok,
        lparen: token(parenthesized, "("),
        specs,
        rparen: last_token(parenthesized, ")"),
    })
}

pub fn spec(converter: &Converter<'_>, node: Node<'_>) -> Option<Spec> {
    match node.kind() {
        "import_spec" => Some(Spec::Import(ImportSpec {
            name: node.child_by_field_name("name").map(|name| converter.ident(name)),
            path: converter.literal(node.child_by_field_name("path")?, Token::String),
            end_pos: Pos::NONE,
            comment: converter.line_comment(node),
        })),
        "const_spec" | "var_spec" => Some(Spec::Value(ValueSpec {
            names: named_field(node, "name")
                .into_iter()
                .map(|name| converter.ident(name))
                .collect(),
            typ: optional_expr(converter, node, "type").ok()?,
            values: match node.child_by_field_name("value") {
                Some(values) => expr_list(converter, values)?,
                None => Vec::new(),
            },
            comment: converter.line_comment(node),
        })),
        "type_spec" | "type_alias" => Some(Spec::Type(TypeSpec {
            name: converter.ident(node.child_by_field_name("name")?),
            type_params: type_parameters(converter, node).ok()?,
            assign: token(node, "="),
            typ: field_expr(converter, node, "type")?,
        })),
        _ => None,
    }
}