use tree_sitter::Node;

use super::expressions::{expression, expression_or_tuple, parenthesized};
use super::{Converter, Outcome, calls, field, has_token, named, only_named, parts, span};
use crate::formatter::python::ast::{Data, Expr, NodeId, Other, Span};

const ASYNC: &str = "async";
const FOR_IN_CLAUSE: &str = "for_in_clause";
const IF_CLAUSE: &str = "if_clause";
const PAIR: &str = "pair";
const DICTIONARY_SPLAT: &str = "dictionary_splat";
const OPENING_PARENTHESIS: &str = "(";
const TUPLE_PATTERN: &str = "tuple_pattern";
const COMMA: &str = ",";

pub fn convert<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree> {
    let added = match node.kind() {
        "list" | "list_pattern" => Expr::List {
            elts: elements(converter, node)?,
        },
        "set" => Expr::Set {
            elts: elements(converter, node)?,
        },
        TUPLE_PATTERN if named(node).len() == 1 && !has_token(node, COMMA) => {
            return parenthesized(converter, node);
        }
        "tuple" | TUPLE_PATTERN => Expr::Tuple {
            elts: elements(converter, node)?,
            parenthesized: true,
        },
        "dictionary" => dictionary(converter, node)?,
        "list_comprehension" => Expr::ListComp {
            elt: expression(converter, field(node, "body")?)?,
            generators: generators(converter, node)?,
        },
        "set_comprehension" => Expr::SetComp {
            elt: expression(converter, field(node, "body")?)?,
            generators: generators(converter, node)?,
        },
        "dictionary_comprehension" => {
            let pair = field(node, "body")?;
            Expr::DictComp {
                key: expression(converter, field(pair, "key")?)?,
                value: expression(converter, field(pair, "value")?)?,
                generators: generators(converter, node)?,
            }
        }
        "generator_expression" => return generator(converter, node, span(node)),
        _ => return calls::convert(converter, node),
    };
    Ok(converter.expression(node, added))
}

fn elements<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree, Vec<NodeId>> {
    named(node)
        .into_iter()
        .map(|element| expression(converter, element))
        .collect()
}

fn dictionary<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree, Expr> {
    let mut items = Vec::new();
    for item in named(node) {
        match item.kind() {
            PAIR => items.push((
                Some(expression(converter, field(item, "key")?)?),
                expression(converter, field(item, "value")?)?,
            )),
            DICTIONARY_SPLAT => items.push((None, expression(converter, only_named(item)?)?)),
            _ => return Err(item),
        }
    }
    Ok(Expr::Dict { items })
}

// El generador que es el único argumento de una llamada comparte los paréntesis de la llamada: su
// tramo es el de adentro y no cuenta como parentizado.
pub fn generator<'tree>(
    converter: &mut Converter<'_>,
    node: Node<'tree>,
    generator_span: Span,
) -> Outcome<'tree> {
    let generator = Expr::Generator {
        elt: expression(converter, field(node, "body")?)?,
        generators: generators(converter, node)?,
        parenthesized: generator_span == span(node) && has_token(node, OPENING_PARENTHESIS),
    };
    Ok(converter.ast.add(generator_span, Data::Expr(generator)))
}

#[derive(Debug)]
struct PendingComprehension {
    start: usize,
    end: usize,
    is_async: bool,
    target: NodeId,
    iter: NodeId,
    ifs: Vec<NodeId>,
}

fn finish(converter: &mut Converter<'_>, pending: PendingComprehension) -> NodeId {
    let comprehension = Other::Comprehension {
        is_async: pending.is_async,
        target: pending.target,
        iter: pending.iter,
        ifs: pending.ifs,
    };
    converter
        .ast
        .add(Span::new(pending.start, pending.end), Data::Other(comprehension))
}

fn for_in<'tree>(converter: &mut Converter<'_>, clause: Node<'tree>) -> Outcome<'tree, PendingComprehension> {
    let mut cursor = clause.walk();
    let iterated: Vec<Node<'tree>> = clause
        .children_by_field_name("right", &mut cursor)
        .filter(Node::is_named)
        .collect();
    let [iter] = iterated.as_slice() else {
        return Err(clause);
    };
    Ok(PendingComprehension {
        start: clause.start_byte(),
        end: clause.end_byte(),
        is_async: has_token(clause, ASYNC),
        target: expression_or_tuple(converter, field(clause, "left")?)?,
        iter: expression(converter, *iter)?,
        ifs: Vec::new(),
    })
}

// Cada `for ... in ...` abre una cláusula y los `if` que le siguen son sus condiciones.
fn generators<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree, Vec<NodeId>> {
    let mut finished = Vec::new();
    let mut pending: Option<PendingComprehension> = None;
    for clause in parts(node) {
        match clause.kind() {
            FOR_IN_CLAUSE => {
                if let Some(previous) = pending.take() {
                    finished.push(finish(converter, previous));
                }
                pending = Some(for_in(converter, clause)?);
            }
            IF_CLAUSE => {
                let condition = expression(converter, only_named(clause)?)?;
                let current = pending.as_mut().ok_or(clause)?;
                current.ifs.push(condition);
                current.end = clause.end_byte();
            }
            _ => {}
        }
    }
    finished.extend(pending.map(|last| finish(converter, last)));
    if finished.is_empty() {
        return Err(node);
    }
    Ok(finished)
}
