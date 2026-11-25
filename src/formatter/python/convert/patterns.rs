use tree_sitter::Node;

use super::expressions::{element_or_tuple, expression};
use super::pattern_classes;
use super::statements::block;
use super::{Converter, Outcome, field, has_token, named, only_named, parts, span, strings};
use crate::formatter::python::ast::{
    Data, Expr, NodeId, Operator, Other, SequenceKind, Singleton, Span, Stmt, UnaryOperator,
};

const CASE_PATTERN: &str = "case_pattern";
const AS_PATTERN: &str = "as_pattern";
const UNION_PATTERN: &str = "union_pattern";
const LIST_PATTERN: &str = "list_pattern";
const TUPLE_PATTERN: &str = "tuple_pattern";
const DICT_PATTERN: &str = "dict_pattern";
const CLASS_PATTERN: &str = "class_pattern";
const SPLAT_PATTERN: &str = "splat_pattern";
const COMPLEX_PATTERN: &str = "complex_pattern";
const DOTTED_NAME: &str = "dotted_name";
const IF_CLAUSE: &str = "if_clause";
const CASE_CLAUSE: &str = "case_clause";
const IDENTIFIER: &str = "identifier";
const WILDCARD: &str = "_";
const COMMA: &str = ",";
const UNION_SEPARATOR: &str = "|";
const NUMBERS: [&str; 2] = ["integer", "float"];
const STRINGS: [&str; 2] = ["string", "concatenated_string"];
const LITERALS: [&str; 3] = ["none", "true", "false"];

pub(super) fn add_other(converter: &mut Converter<'_>, bounds: Span, other: Other) -> NodeId {
    converter.ast.add(bounds, Data::Other(other))
}

fn add_expression(converter: &mut Converter<'_>, bounds: Span, expression: Expr) -> NodeId {
    converter.ast.add(bounds, Data::Expr(expression))
}

pub fn match_statement<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree, Stmt> {
    let mut subjects = Vec::new();
    let mut body = None;
    let mut cursor = node.walk();
    for child in node.children_by_field_name("subject", &mut cursor) {
        subjects.push(child);
    }
    if let Some(block_node) = node.child_by_field_name("body") {
        body = Some(block_node);
    }
    let subject = element_or_tuple(converter, node, &subjects)?;
    let mut cases = Vec::new();
    for case in named(body.ok_or(node)?) {
        if case.kind() != CASE_CLAUSE {
            return Err(case);
        }
        cases.push(case_clause(converter, case)?);
    }
    Ok(Stmt::Match { subject, cases })
}

fn case_clause<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree> {
    let mut case_patterns = Vec::new();
    let mut guard = None;
    for child in named(node) {
        match child.kind() {
            CASE_PATTERN => case_patterns.push(child),
            IF_CLAUSE => guard = Some(expression(converter, only_named(child)?)?),
            _ => {}
        }
    }
    let pattern = top_level_pattern(converter, node, &case_patterns)?;
    let body = block(converter, field(node, "consequence")?)?;
    Ok(add_other(
        converter,
        span(node),
        Other::MatchCase { pattern, guard, body },
    ))
}

fn top_level_pattern<'tree>(
    converter: &mut Converter<'_>,
    case: Node<'tree>,
    case_patterns: &[Node<'tree>],
) -> Outcome<'tree> {
    let (Some(first), Some(last)) = (case_patterns.first(), case_patterns.last()) else {
        return Err(case);
    };
    if let [only] = case_patterns
        && !has_token(case, COMMA)
    {
        return pattern(converter, *only);
    }
    let end = trailing_comma_end(case, *last);
    sequence(
        converter,
        case_patterns,
        Span::new(first.start_byte(), end),
        SequenceKind::TupleWithoutParentheses,
    )
}

fn trailing_comma_end(container: Node<'_>, last: Node<'_>) -> usize {
    parts(container)
        .into_iter()
        .find(|part| part.kind() == COMMA && part.start_byte() >= last.end_byte())
        .map_or_else(|| last.end_byte(), |comma| comma.end_byte())
}

fn sequence<'tree>(
    converter: &mut Converter<'_>,
    elements: &[Node<'tree>],
    bounds: Span,
    kind: SequenceKind,
) -> Outcome<'tree> {
    let patterns = elements
        .iter()
        .map(|element| pattern(converter, *element))
        .collect::<Outcome<'tree, Vec<NodeId>>>()?;
    Ok(add_other(
        converter,
        bounds,
        Other::PatternSequence { patterns, kind },
    ))
}

fn sign_of(converter: &Converter<'_>, node: Node<'_>) -> Option<UnaryOperator> {
    match converter.text(node) {
        "-" if !node.is_named() => Some(UnaryOperator::USub),
        "+" if !node.is_named() => Some(UnaryOperator::UAdd),
        _ => None,
    }
}

pub(super) fn dotted_name<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree> {
    let names: Vec<Node<'tree>> = named(node);
    let (first, rest) = names.split_first().ok_or(node)?;
    let mut current = add_expression(converter, span(*first), Expr::Name);
    for attribute in rest {
        let bounds = Span::new(first.start_byte(), attribute.end_byte());
        current = add_expression(
            converter,
            bounds,
            Expr::Attribute {
                value: current,
                attr: span(*attribute),
            },
        );
    }
    Ok(current)
}

fn number<'tree>(
    converter: &mut Converter<'_>,
    sign: Option<Node<'tree>>,
    number: Node<'tree>,
) -> Outcome<'tree> {
    let value = expression(converter, number)?;
    let Some(sign) = sign else {
        return Ok(value);
    };
    let op = sign_of(converter, sign).ok_or(sign)?;
    Ok(add_expression(
        converter,
        Span::new(sign.start_byte(), number.end_byte()),
        Expr::UnaryOp { op, operand: value },
    ))
}

fn complex<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree> {
    let children = parts(node);
    let (sign, rest) = match children.first() {
        Some(first) if sign_of(converter, *first).is_some() => {
            (Some(*first), children.get(1..).unwrap_or_default())
        }
        Some(_) | None => (None, children.as_slice()),
    };
    let [real, operator, imaginary] = rest else {
        return Err(node);
    };
    let left = number(converter, sign, *real)?;
    let right = expression(converter, *imaginary)?;
    let op = if converter.text(*operator) == "-" {
        Operator::Sub
    } else {
        Operator::Add
    };
    Ok(add_expression(
        converter,
        span(node),
        Expr::BinOp { left, op, right },
    ))
}

// Un valor que va como clave o como patrón de valor: un número con signo, una cadena, un nombre
// con puntos, `None`, `True` o `False`.
pub(super) fn value_expression<'tree>(
    converter: &mut Converter<'_>,
    parent: Node<'tree>,
    children: &[Node<'tree>],
) -> Outcome<'tree> {
    match children {
        [single] if STRINGS.contains(&single.kind()) => strings::convert(converter, *single),
        [single] if NUMBERS.contains(&single.kind()) || LITERALS.contains(&single.kind()) => {
            expression(converter, *single)
        }
        [single] if single.kind() == DOTTED_NAME => dotted_name(converter, *single),
        [single] if single.kind() == COMPLEX_PATTERN => complex(converter, *single),
        [sign, value] if NUMBERS.contains(&value.kind()) => number(converter, Some(*sign), *value),
        [first, ..] => Err(*first),
        [] => Err(parent),
    }
}

fn singleton_of(kind: &str) -> Option<Singleton> {
    match kind {
        "none" => Some(Singleton::None),
        "true" => Some(Singleton::True),
        "false" => Some(Singleton::False),
        _ => None,
    }
}

pub(super) fn value_pattern<'tree>(
    converter: &mut Converter<'_>,
    parent: Node<'tree>,
    children: &[Node<'tree>],
) -> Outcome<'tree> {
    if let [single] = children {
        if let Some(singleton) = singleton_of(single.kind()) {
            return Ok(add_other(
                converter,
                span(*single),
                Other::PatternSingleton(singleton),
            ));
        }
        if single.kind() == DOTTED_NAME && named(*single).len() == 1 {
            let name = named(*single).first().map(|identifier| span(*identifier));
            return Ok(add_other(
                converter,
                span(*single),
                Other::PatternAs { pattern: None, name },
            ));
        }
    }
    let value = value_expression(converter, parent, children)?;
    let bounds = converter.ast.span(value);
    Ok(add_other(converter, bounds, Other::PatternValue { value }))
}

pub(super) fn pattern<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree> {
    match node.kind() {
        CASE_PATTERN => case_pattern(converter, node),
        AS_PATTERN => as_pattern(converter, node),
        UNION_PATTERN => union(converter, node),
        LIST_PATTERN => bracketed(converter, node, SequenceKind::List),
        TUPLE_PATTERN => tuple(converter, node),
        DICT_PATTERN => pattern_classes::mapping(converter, node),
        CLASS_PATTERN => pattern_classes::class(converter, node),
        SPLAT_PATTERN => Ok(star(converter, node)),
        _ => value_pattern(converter, node, &[node]),
    }
}

fn case_pattern<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree> {
    let children = parts(node);
    match children.as_slice() {
        [only] if !only.is_named() && converter.text(*only) == WILDCARD => Ok(add_other(
            converter,
            span(*only),
            Other::PatternAs {
                pattern: None,
                name: None,
            },
        )),
        [only] => pattern(converter, *only),
        _ => value_pattern(converter, node, &children),
    }
}

fn as_pattern<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree> {
    let children = named(node);
    let [inner, name] = children.as_slice() else {
        return Err(node);
    };
    if name.kind() != IDENTIFIER {
        return Err(*name);
    }
    let inner = pattern(converter, *inner)?;
    Ok(add_other(
        converter,
        span(node),
        Other::PatternAs {
            pattern: Some(inner),
            name: Some(span(*name)),
        },
    ))
}

fn union_items<'tree>(node: Node<'tree>) -> Vec<Vec<Node<'tree>>> {
    let mut groups: Vec<Vec<Node<'tree>>> = vec![Vec::new()];
    for child in parts(node) {
        if !child.is_named() && child.kind() == UNION_SEPARATOR {
            groups.push(Vec::new());
        } else if let Some(current) = groups.last_mut() {
            current.push(child);
        }
    }
    groups
}

fn union<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree> {
    let mut patterns = Vec::new();
    for group in union_items(node) {
        patterns.push(match group.as_slice() {
            [single] => pattern(converter, *single)?,
            _ => value_pattern(converter, node, &group)?,
        });
    }
    Ok(add_other(converter, span(node), Other::PatternOr { patterns }))
}

pub(super) fn case_patterns(node: Node<'_>) -> Vec<Node<'_>> {
    named(node)
        .into_iter()
        .filter(|child| child.kind() == CASE_PATTERN)
        .collect()
}

fn bracketed<'tree>(converter: &mut Converter<'_>, node: Node<'tree>, kind: SequenceKind) -> Outcome<'tree> {
    sequence(converter, &case_patterns(node), span(node), kind)
}

fn tuple<'tree>(converter: &mut Converter<'_>, node: Node<'tree>) -> Outcome<'tree> {
    let elements = case_patterns(node);
    if let [only] = elements.as_slice()
        && !has_token(node, COMMA)
    {
        return pattern(converter, *only);
    }
    sequence(converter, &elements, span(node), SequenceKind::Tuple)
}

fn star(converter: &mut Converter<'_>, node: Node<'_>) -> NodeId {
    let name = named(node).first().map(|identifier| span(*identifier));
    add_other(converter, span(node), Other::PatternStar { name })
}
