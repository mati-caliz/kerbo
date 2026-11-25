use tree_sitter::Node;

use super::calls::arguments;
use super::expressions::{expression, expression_or_tuple};
use super::statements::block;
use super::{
    Converter, Outcome, field, has_token, named, only_named, parameters, patterns, span, type_params, types,
};
use crate::formatter::python::ast::{Data, NodeId, Other, Span, Stmt};

const ASYNC: &str = "async";
const STAR: &str = "*";
const OPENING_PARENTHESIS: &str = "(";
const COMMA: &str = ",";
const ELSE_CLAUSE: &str = "else_clause";
const AS_PATTERN: &str = "as_pattern";
const TUPLE: &str = "tuple";

pub fn statement<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree> {
    let added = match node.kind() {
        "function_definition" => function(converter, node, Vec::new())?,
        "class_definition" => class(converter, node, Vec::new())?,
        "decorated_definition" => decorated(converter, node)?,
        "if_statement" => if_statement(converter, node)?,
        "for_statement" => for_statement(converter, node)?,
        "while_statement" => Stmt::While {
            test: expression(converter, field(node, "condition")?)?,
            body: block(converter, field(node, "body")?)?,
            orelse: else_body(converter, node)?,
        },
        "try_statement" => try_statement(converter, node)?,
        "match_statement" => patterns::match_statement(converter, node)?,
        "with_statement" => with_statement(converter, node)?,
        _ => return Err(node),
    };
    Ok(converter.statement(node, added))
}

fn function<'tree>(
    converter: &mut Converter<'_>,
    node: Node<'tree>,
    decorators: Vec<NodeId>,
) -> Outcome<'tree, Stmt> {
    let type_params = type_params::optional(converter, node)?;
    let returns = node
        .child_by_field_name("return_type")
        .map(|annotation| types::annotation(converter, annotation))
        .transpose()?;
    Ok(Stmt::FunctionDef {
        is_async: has_token(node, ASYNC),
        decorators,
        name: span(field(node, "name")?),
        type_params,
        parameters: parameters::convert(converter, field(node, "parameters")?)?,
        returns,
        body: block(converter, field(node, "body")?)?,
    })
}

fn class<'tree>(
    converter: &mut Converter<'_>,
    node: Node<'tree>,
    decorators: Vec<NodeId>,
) -> Outcome<'tree, Stmt> {
    let type_params = type_params::optional(converter, node)?;
    let superclasses = node
        .child_by_field_name("superclasses")
        .map(|superclasses| arguments(converter, superclasses))
        .transpose()?;
    Ok(Stmt::ClassDef {
        decorators,
        name: span(field(node, "name")?),
        type_params,
        arguments: superclasses,
        body: block(converter, field(node, "body")?)?,
    })
}

// La definición decorada es un solo nodo, que empieza en el primer decorador.
fn decorated<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree, Stmt> {
    let definition = field(node, "definition")?;
    let mut decorators = Vec::new();
    for child in named(node) {
        if child == definition {
            continue;
        }
        let expression = expression(converter, only_named(child)?)?;
        decorators.push(converter.other(child, Other::Decorator { expression }));
    }
    if definition.kind() == "class_definition" {
        class(converter, definition, decorators)
    } else {
        function(converter, definition, decorators)
    }
}

fn alternatives(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.children_by_field_name("alternative", &mut cursor).collect()
}

fn else_body<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree, Vec<NodeId>> {
    match node.child_by_field_name("alternative") {
        Some(alternative) => block(converter, field(alternative, "body")?),
        None => Ok(Vec::new()),
    }
}

fn if_statement<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree, Stmt> {
    let test = expression(converter, field(node, "condition")?)?;
    let body = block(converter, field(node, "consequence")?)?;
    let mut clauses = Vec::new();
    for clause in alternatives(node) {
        let added = if clause.kind() == ELSE_CLAUSE {
            Other::ElifElseClause {
                test: None,
                body: block(converter, field(clause, "body")?)?,
            }
        } else {
            Other::ElifElseClause {
                test: Some(expression(converter, field(clause, "condition")?)?),
                body: block(converter, field(clause, "consequence")?)?,
            }
        };
        clauses.push(converter.other(clause, added));
    }
    Ok(Stmt::If { test, body, clauses })
}

fn for_statement<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree, Stmt> {
    Ok(Stmt::For {
        is_async: has_token(node, ASYNC),
        target: expression_or_tuple(converter, field(node, "left")?)?,
        iter: expression_or_tuple(converter, field(node, "right")?)?,
        body: block(converter, field(node, "body")?)?,
        orelse: else_body(converter, node)?,
    })
}

fn except_handler<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree> {
    let mut cursor = node.walk();
    let values: Vec<Node<'tree>> = node.children_by_field_name("value", &mut cursor).collect();
    let mut name = node.child_by_field_name("alias").map(span);
    let type_ = match values.as_slice() {
        [] => None,
        [exception] if exception.kind() == AS_PATTERN => {
            let alias = field(*exception, "alias")?;
            name = Some(span(only_named(alias)?));
            let caught = named(*exception)
                .into_iter()
                .find(|child| *child != alias)
                .ok_or(*exception)?;
            Some(expression(converter, caught)?)
        }
        [exception] => Some(expression(converter, *exception)?),
        _ => return Err(node),
    };
    let body_node = named(node)
        .into_iter()
        .rfind(|child| child.kind() == "block")
        .ok_or(node)?;
    let body = block(converter, body_node)?;
    Ok(converter.other(node, Other::ExceptHandler { type_, name, body }))
}

fn try_statement<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree, Stmt> {
    let body = block(converter, field(node, "body")?)?;
    let mut handlers = Vec::new();
    let mut orelse = Vec::new();
    let mut finalbody = Vec::new();
    let mut is_star = false;
    for clause in named(node) {
        match clause.kind() {
            "except_clause" => {
                is_star = is_star || has_token(clause, STAR);
                handlers.push(except_handler(converter, clause)?);
            }
            ELSE_CLAUSE => orelse = block(converter, field(clause, "body")?)?,
            "finally_clause" => finalbody = block(converter, only_named(clause)?)?,
            _ => {}
        }
    }
    Ok(Stmt::Try {
        body,
        handlers,
        orelse,
        finalbody,
        is_star,
    })
}

fn with_item<'tree>(converter: &mut Converter<'_>, node: Node<'tree>, item_span: Span) -> Outcome<'tree> {
    let item = if node.kind() == AS_PATTERN {
        let alias = field(node, "alias")?;
        let context = named(node)
            .into_iter()
            .find(|child| *child != alias)
            .ok_or(node)?;
        Other::WithItem {
            context_expr: expression(converter, context)?,
            optional_vars: Some(expression(converter, only_named(alias)?)?),
        }
    } else {
        Other::WithItem {
            context_expr: expression(converter, node)?,
            optional_vars: None,
        }
    };
    Ok(converter.ast.add(item_span, Data::Other(item)))
}

// `with (a, b):` son dos ítems entre paréntesis y no una tupla, como lo lee Python desde 3.9;
// tree-sitter a veces lo deja como un solo ítem con una tupla adentro.
fn with_items<'tree>(converter: &mut Converter<'_>, clause: Node<'tree>) -> Outcome<'tree, Vec<NodeId>> {
    let items = named(clause);
    if let [item] = items.as_slice()
        && !has_token(clause, OPENING_PARENTHESIS)
    {
        let value = field(*item, "value")?;
        let elements = named(value);
        let is_item_list = value.kind() == TUPLE && (elements.len() > 1 || has_token(value, COMMA));
        if is_item_list {
            return elements
                .into_iter()
                .map(|element| with_item(converter, element, span(element)))
                .collect();
        }
    }
    items
        .into_iter()
        .map(|item| with_item(converter, field(item, "value")?, span(item)))
        .collect()
}

fn with_statement<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree, Stmt> {
    let clause = named(node)
        .into_iter()
        .find(|child| child.kind() == "with_clause")
        .ok_or(node)?;
    Ok(Stmt::With {
        is_async: has_token(node, ASYNC),
        items: with_items(converter, clause)?,
        body: block(converter, field(node, "body")?)?,
    })
}
