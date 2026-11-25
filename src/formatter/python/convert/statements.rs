use tree_sitter::Node;

use super::expressions::{expression, expression_or_tuple, tuple_of};
use super::{Converter, Outcome, compound, field, has_token, named, parts, span, token, type_params, types};
use crate::formatter::python::ast::{NodeId, Operator, Other, Span, Stmt};

const COMMA: &str = ",";
const LAZY: &str = "lazy";
const FUTURE: &str = "__future__";
const DOT: char = '.';
const ASSIGNMENT: &str = "assignment";
const ALIASED_IMPORT: &str = "aliased_import";
const WILDCARD_IMPORT: &str = "wildcard_import";
const RELATIVE_IMPORT: &str = "relative_import";
const IMPORT_PREFIX: &str = "import_prefix";
const DOTTED_NAME: &str = "dotted_name";

pub fn block<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree, Vec<NodeId>> {
    named(node)
        .into_iter()
        .map(|child| statement(converter, child))
        .collect()
}

pub fn statement<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree> {
    let added = match node.kind() {
        "expression_statement" => return expression_statement(converter, node),
        "return_statement" => Stmt::Return {
            value: optional_value(converter, node)?,
        },
        "delete_statement" => delete(converter, node)?,
        "raise_statement" => raise(converter, node)?,
        "assert_statement" => assert(converter, node)?,
        "type_alias_statement" => type_params::alias(converter, node)?,
        "pass_statement" => Stmt::Pass,
        "break_statement" => Stmt::Break,
        "continue_statement" => Stmt::Continue,
        "global_statement" => Stmt::Global {
            names: named(node).into_iter().map(span).collect(),
        },
        "nonlocal_statement" => Stmt::Nonlocal {
            names: named(node).into_iter().map(span).collect(),
        },
        "import_statement" => Stmt::Import {
            names: aliases(converter, node)?,
            is_lazy: has_token(node, LAZY),
        },
        "import_from_statement" | "future_import_statement" => import_from(converter, node)?,
        _ => return compound::statement(converter, node),
    };
    Ok(converter.statement(node, added))
}

fn optional_value<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree, Option<NodeId>> {
    match named(node).as_slice() {
        [] => Ok(None),
        [value] => expression_or_tuple(converter, *value).map(Some),
        _ => Err(node),
    }
}

fn expression_statement<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree> {
    let children = named(node);
    let value = match children.as_slice() {
        [only] if !has_token(node, COMMA) => match only.kind() {
            ASSIGNMENT => return assignment(converter, node, *only),
            "augmented_assignment" => return augmented_assignment(converter, node, *only),
            _ => expression(converter, *only)?,
        },
        _ => tuple_of(converter, span(node), &children)?,
    };
    Ok(converter.statement(node, Stmt::Expression { value }))
}

fn assignment<'tree>(
    converter: &mut Converter<'_>,
    statement: Node<'tree>,
    node: Node<'tree>,
) -> Outcome<'tree> {
    let target = expression_or_tuple(converter, field(node, "left")?)?;
    if let Some(annotation) = node.child_by_field_name("type") {
        let annotation = types::annotation(converter, annotation)?;
        let value = node
            .child_by_field_name("right")
            .map(|right| expression_or_tuple(converter, right))
            .transpose()?;
        let annotated = Stmt::AnnAssign {
            target,
            annotation,
            value,
        };
        return Ok(converter.statement(statement, annotated));
    }
    let mut targets = vec![target];
    let mut right = field(node, "right")?;
    while right.kind() == ASSIGNMENT {
        if right.child_by_field_name("type").is_some() {
            return Err(right);
        }
        targets.push(expression_or_tuple(converter, field(right, "left")?)?);
        right = field(right, "right")?;
    }
    let value = expression_or_tuple(converter, right)?;
    Ok(converter.statement(statement, Stmt::Assign { targets, value }))
}

fn augmented_assignment<'tree>(
    converter: &mut Converter<'_>,
    statement: Node<'tree>,
    node: Node<'tree>,
) -> Outcome<'tree> {
    let target = expression(converter, field(node, "left")?)?;
    let operator = field(node, "operator")?;
    let op = Operator::from_token(converter.text(operator)).ok_or(operator)?;
    let value = expression_or_tuple(converter, field(node, "right")?)?;
    Ok(converter.statement(statement, Stmt::AugAssign { target, op, value }))
}

// `del a, b` borra dos destinos; `del (a, b)` borra una tupla.
fn delete<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree, Stmt> {
    let children = named(node);
    let targets = match children.as_slice() {
        [list] if list.kind() == "expression_list" => named(*list),
        _ => children,
    };
    let targets = targets
        .into_iter()
        .map(|target| expression(converter, target))
        .collect::<Outcome<'tree, Vec<NodeId>>>()?;
    Ok(Stmt::Delete { targets })
}

fn raise<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree, Stmt> {
    let cause = node.child_by_field_name("cause");
    let raised: Vec<Node<'tree>> = named(node)
        .into_iter()
        .filter(|child| Some(*child) != cause)
        .collect();
    let exc = match raised.as_slice() {
        [] => None,
        [exception] => Some(expression(converter, *exception)?),
        _ => return Err(node),
    };
    let cause = cause.map(|cause| expression(converter, cause)).transpose()?;
    Ok(Stmt::Raise { exc, cause })
}

fn assert<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree, Stmt> {
    match named(node).as_slice() {
        [test] => Ok(Stmt::Assert {
            test: expression(converter, *test)?,
            msg: None,
        }),
        [test, message] => Ok(Stmt::Assert {
            test: expression(converter, *test)?,
            msg: Some(expression(converter, *message)?),
        }),
        _ => Err(node),
    }
}

fn alias<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree> {
    let alias = match node.kind() {
        ALIASED_IMPORT => Other::Alias {
            name: span(field(node, "name")?),
            asname: Some(span(field(node, "alias")?)),
        },
        DOTTED_NAME | WILDCARD_IMPORT => Other::Alias {
            name: span(node),
            asname: None,
        },
        _ => return Err(node),
    };
    Ok(converter.other(node, alias))
}

fn aliases<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree, Vec<NodeId>> {
    let mut cursor = node.walk();
    let wildcard = named(node)
        .into_iter()
        .filter(|child| child.kind() == WILDCARD_IMPORT);
    node.children_by_field_name("name", &mut cursor)
        .chain(wildcard)
        .map(|name| alias(converter, name))
        .collect()
}

// El módulo de un `from`: cuántos puntos tiene adelante y el tramo del nombre, si lo hay.
fn imported_module<'tree>(
    converter: &Converter<'_>,
    node: Node<'tree>,
) -> Outcome<'tree, (usize, Option<Span>)> {
    let Some(module) = node.child_by_field_name("module_name") else {
        let future = token(node, FUTURE).ok_or(node)?;
        return Ok((0, Some(span(future))));
    };
    if module.kind() != RELATIVE_IMPORT {
        return Ok((0, Some(span(module))));
    }
    let mut level = 0_usize;
    let mut name = None;
    for part in parts(module) {
        match part.kind() {
            IMPORT_PREFIX => {
                level = converter
                    .text(part)
                    .chars()
                    .filter(|character| *character == DOT)
                    .count();
            }
            DOTTED_NAME => name = Some(span(part)),
            _ => return Err(part),
        }
    }
    Ok((level, name))
}

fn import_from<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree, Stmt> {
    let (level, module) = imported_module(converter, node)?;
    Ok(Stmt::ImportFrom {
        module,
        names: aliases(converter, node)?,
        level,
        is_lazy: has_token(node, LAZY),
    })
}
