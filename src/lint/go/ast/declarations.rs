use tree_sitter::Node;

use super::convert::{Converter, children, field_nodes, missing, unary_other};
use super::{Decl, DeclKind, Expr, Field, FuncDecl, FuncType, GenDecl, Spec, Stmt, statements};

const POINTER_MARK: &str = "*";

pub fn top_level(converter: &mut Converter<'_>, node: Node<'_>) -> Option<Decl> {
    match node.kind() {
        "function_declaration" | "method_declaration" => Some(Decl::Func(func_decl(converter, node))),
        "import_declaration" => Some(Decl::Gen(GenDecl {
            kind: DeclKind::Import,
            specs: Vec::new(),
        })),
        _ => gen_decl(converter, node).map(Decl::Gen),
    }
}

pub fn gen_decl(converter: &mut Converter<'_>, node: Node<'_>) -> Option<GenDecl> {
    let kind = match node.kind() {
        "var_declaration" => DeclKind::Var,
        "const_declaration" => DeclKind::Const,
        "type_declaration" => DeclKind::Type,
        _ => return None,
    };
    let specs = spec_nodes(node)
        .into_iter()
        .map(|node| spec(converter, node))
        .collect();
    Some(GenDecl { kind, specs })
}

fn spec(converter: &mut Converter<'_>, node: Node<'_>) -> Spec {
    match node.kind() {
        "type_spec" | "type_alias" => Spec::Type {
            name: converter.field_ident(node, "name"),
            type_params: node
                .child_by_field_name("type_parameters")
                .map(|list| fields(converter, list)),
            typ: converter.field_expr(node, "type").unwrap_or_else(missing),
        },
        _ => Spec::Value {
            names: field_nodes(node, "name")
                .into_iter()
                .map(|name| converter.ident(name))
                .collect(),
            typ: converter.field_expr(node, "type"),
            values: converter.list(node.child_by_field_name("value")),
        },
    }
}

fn func_decl(converter: &mut Converter<'_>, node: Node<'_>) -> FuncDecl {
    FuncDecl {
        receiver: node
            .child_by_field_name("receiver")
            .map(|list| fields(converter, list)),
        name: converter.field_ident(node, "name"),
        typ: func_type(converter, node),
        body: node
            .child_by_field_name("body")
            .map(|body| statements::block(converter, body)),
    }
}

pub fn func_type(converter: &mut Converter<'_>, node: Node<'_>) -> FuncType {
    FuncType {
        type_params: node
            .child_by_field_name("type_parameters")
            .map(|list| fields(converter, list)),
        params: node
            .child_by_field_name("parameters")
            .map(|list| fields(converter, list))
            .unwrap_or_default(),
        results: node.child_by_field_name("result").map(|result| {
            if result.kind() == "parameter_list" {
                fields(converter, result)
            } else {
                vec![Field {
                    names: Vec::new(),
                    typ: Some(converter.expr(result)),
                }]
            }
        }),
    }
}

pub fn body(converter: &mut Converter<'_>, node: Node<'_>) -> Vec<Stmt> {
    node.child_by_field_name("body")
        .map(|body| statements::block(converter, body))
        .unwrap_or_default()
}

pub fn fields(converter: &mut Converter<'_>, list: Node<'_>) -> Vec<Field> {
    children(list)
        .into_iter()
        .map(|declaration| field(converter, declaration))
        .collect()
}

fn field(converter: &mut Converter<'_>, node: Node<'_>) -> Field {
    let names = field_nodes(node, "name")
        .into_iter()
        .map(|name| converter.ident(name))
        .collect::<Vec<_>>();
    let typ = match node.kind() {
        "variadic_parameter_declaration" => Expr::Ellipsis(Some(converter.boxed_field(node, "type"))),
        "method_elem" => Expr::FuncType(func_type(converter, node)),
        "type_elem" => converter.union(node),
        "field_declaration" if names.is_empty() && has_pointer_mark(converter, node) => {
            Expr::Star(converter.boxed_field(node, "type"))
        }
        _ => converter.field_expr(node, "type").unwrap_or_else(missing),
    };
    Field {
        names,
        typ: Some(typ),
    }
}

fn has_pointer_mark(converter: &Converter<'_>, node: Node<'_>) -> bool {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .any(|child| !child.is_named() && converter.text(child) == POINTER_MARK)
}

pub fn type_expr(converter: &mut Converter<'_>, node: Node<'_>) -> Option<Expr> {
    Some(match node.kind() {
        "pointer_type" => Expr::Star(Box::new(converter.first(node))),
        "qualified_type" => Expr::Selector {
            operand: converter.boxed_field(node, "package"),
            field: converter.field_ident(node, "name"),
        },
        "generic_type" => Expr::Index {
            operand: converter.boxed_field(node, "type"),
            indices: node
                .child_by_field_name("type_arguments")
                .map(|arguments| converter.exprs(children(arguments)))
                .unwrap_or_default(),
        },
        "type_elem" | "type_constraint" => converter.union(node),
        "array_type" => Expr::ArrayType {
            length: Some(converter.boxed_field(node, "length")),
            element: converter.boxed_field(node, "element"),
        },
        "implicit_length_array_type" => Expr::ArrayType {
            length: Some(Box::new(Expr::Ellipsis(None))),
            element: converter.boxed_field(node, "element"),
        },
        "slice_type" => Expr::ArrayType {
            length: None,
            element: converter.boxed_field(node, "element"),
        },
        "map_type" => Expr::MapType {
            key: converter.boxed_field(node, "key"),
            value: converter.boxed_field(node, "value"),
        },
        "channel_type" => Expr::ChanType(converter.boxed_field(node, "value")),
        "function_type" => Expr::FuncType(func_type(converter, node)),
        "struct_type" => Expr::StructType(
            children(node)
                .first()
                .map(|list| fields(converter, *list))
                .unwrap_or_default(),
        ),
        "interface_type" => Expr::InterfaceType(fields(converter, node)),
        "negated_type" => unary_other(Box::new(converter.first(node))),
        _ => return None,
    })
}

fn spec_nodes(node: Node<'_>) -> Vec<Node<'_>> {
    children(node)
        .into_iter()
        .flat_map(|child| {
            if child.kind().ends_with("_spec_list") {
                children(child)
            } else {
                vec![child]
            }
        })
        .collect()
}
