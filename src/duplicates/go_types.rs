use tree_sitter::Node;

use super::go_kinds::{
    ARRAY_TYPE, CALL_EXPR, COMPOSITE_LIT, ELLIPSIS, FIELD, FIELD_LIST, FUNC_LIT, FUNC_TYPE, IDENT,
    INDEX_EXPR, INDEX_LIST_EXPR, INTERFACE_TYPE, STAR_EXPR, STRUCT_TYPE,
};
use super::go_statements;
use super::go_syntax::{Builder, field, signature_span, significant_children, span, token, token_start};

const PARAMETER_LIST: &str = "parameter_list";

pub fn composite_expression(builder: &mut Builder<'_>, node: Node<'_>) {
    match node.kind() {
        "generic_type" => instantiation(
            builder,
            span(node),
            field(node, "type"),
            field(node, "type_arguments"),
        ),
        "type_instantiation_expression" => {
            instantiation(builder, span(node), field(node, "type"), Some(node));
        }
        "implicit_length_array_type" => implicit_length_array(builder, node),
        "call_expression" => call(builder, node),
        "composite_literal" => composite(builder, node, field(node, "type"), field(node, "body")),
        "literal_value" => composite(builder, node, None, Some(node)),
        "func_literal" => func_literal(builder, node),
        "function_type" => func_type(builder, span(node), None, node),
        "struct_type" => struct_type(builder, node),
        "interface_type" => interface_type(builder, node),
        _ => builder.unknown(node),
    }
}

fn instantiation(
    builder: &mut Builder<'_>,
    span: (usize, usize),
    operand: Option<Node<'_>>,
    arguments: Option<Node<'_>>,
) {
    let arguments: Vec<Node<'_>> = arguments
        .map(significant_children)
        .unwrap_or_default()
        .into_iter()
        .filter(|argument| operand.is_none_or(|operand| operand.id() != argument.id()))
        .collect();
    let kind = if arguments.len() == 1 {
        INDEX_EXPR
    } else {
        INDEX_LIST_EXPR
    };
    builder.wrap(kind, span, |inner| {
        inner.optional(operand);
        for argument in arguments {
            inner.expression(argument);
        }
    });
}

fn implicit_length_array(builder: &mut Builder<'_>, node: Node<'_>) {
    builder.wrap(ARRAY_TYPE, span(node), |inner| {
        if let Some(ellipsis) = token(node, "...") {
            inner.leaf(ELLIPSIS, ellipsis);
        }
        inner.optional(field(node, "element"));
    });
}

fn callee(builder: &mut Builder<'_>, node: Node<'_>) {
    let function = field(node, "function");
    match (function, field(node, "type_arguments")) {
        (Some(called), Some(arguments)) => {
            instantiation(
                builder,
                (called.start_byte(), arguments.end_byte()),
                Some(called),
                Some(arguments),
            );
        }
        _ => builder.optional(function),
    }
}

fn call(builder: &mut Builder<'_>, node: Node<'_>) {
    builder.wrap(CALL_EXPR, span(node), |inner| {
        callee(inner, node);
        inner.expressions(field(node, "arguments"));
    });
}

fn composite(builder: &mut Builder<'_>, node: Node<'_>, type_node: Option<Node<'_>>, body: Option<Node<'_>>) {
    builder.wrap(COMPOSITE_LIT, span(node), |inner| {
        inner.optional(type_node);
        inner.expressions(body);
    });
}

fn func_literal(builder: &mut Builder<'_>, node: Node<'_>) {
    builder.wrap(FUNC_LIT, span(node), |inner| {
        func_type(inner, signature_span(node, node.start_byte()), None, node);
        if let Some(body) = field(node, "body") {
            go_statements::block(inner, body);
        }
    });
}

pub fn func_type(
    builder: &mut Builder<'_>,
    span: (usize, usize),
    type_parameters: Option<Node<'_>>,
    node: Node<'_>,
) {
    builder.wrap(FUNC_TYPE, span, |inner| {
        if let Some(parameters) = type_parameters {
            field_list(inner, parameters);
        }
        if let Some(parameters) = field(node, "parameters") {
            field_list(inner, parameters);
        }
        results(inner, field(node, "result"));
    });
}

fn results(builder: &mut Builder<'_>, result: Option<Node<'_>>) {
    let Some(result) = result else {
        return;
    };
    if result.kind() == PARAMETER_LIST {
        return field_list(builder, result);
    }
    builder.wrap(FIELD_LIST, span(result), |list| {
        list.wrap(FIELD, span(result), |single| single.expression(result));
    });
}

pub fn field_list(builder: &mut Builder<'_>, list: Node<'_>) {
    builder.wrap(FIELD_LIST, span(list), |inner| {
        for item in significant_children(list) {
            list_field(inner, item);
        }
    });
}

fn list_field(builder: &mut Builder<'_>, node: Node<'_>) {
    match node.kind() {
        "method_elem" => builder.wrap(FIELD, span(node), |inner| method(inner, node)),
        "type_elem" => builder.wrap(FIELD, span(node), |inner| inner.expression(node)),
        "variadic_parameter_declaration" => builder.wrap(FIELD, span(node), |inner| variadic(inner, node)),
        _ => builder.wrap(FIELD, span(node), |inner| named_field(inner, node)),
    }
}

fn method(builder: &mut Builder<'_>, node: Node<'_>) {
    builder.optional(field(node, "name"));
    let parameters_start =
        field(node, "parameters").map_or_else(|| node.start_byte(), |list| list.start_byte());
    func_type(builder, signature_span(node, parameters_start), None, node);
}

fn variadic(builder: &mut Builder<'_>, node: Node<'_>) {
    builder.optional(field(node, "name"));
    let start = token_start(node, "...");
    builder.wrap(ELLIPSIS, (start, node.end_byte()), |inner| {
        inner.optional(field(node, "type"));
    });
}

fn named_field(builder: &mut Builder<'_>, node: Node<'_>) {
    let mut cursor = node.walk();
    for name in node.children_by_field_name("name", &mut cursor) {
        builder.leaf(IDENT, name);
    }
    let field_type = field(node, "type");
    match (token(node, "*"), field_type) {
        (Some(star), Some(embedded)) => {
            builder.wrap(STAR_EXPR, (star.start_byte(), embedded.end_byte()), |inner| {
                inner.expression(embedded);
            });
        }
        _ => builder.optional(field_type),
    }
}

fn struct_type(builder: &mut Builder<'_>, node: Node<'_>) {
    builder.wrap(STRUCT_TYPE, span(node), |inner| {
        if let Some(&fields) = significant_children(node).first() {
            field_list(inner, fields);
        }
    });
}

fn interface_type(builder: &mut Builder<'_>, node: Node<'_>) {
    builder.wrap(INTERFACE_TYPE, span(node), |inner| {
        interface_elements(inner, node);
    });
}

fn interface_elements(builder: &mut Builder<'_>, node: Node<'_>) {
    builder.wrap(FIELD_LIST, (token_start(node, "{"), node.end_byte()), |inner| {
        for element in significant_children(node) {
            list_field(inner, element);
        }
    });
}
