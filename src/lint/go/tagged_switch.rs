use tree_sitter::Node;

use super::parentheses::without_parentheses;
use crate::syntax::{descendants, named_children};

const IF_STATEMENT: &str = "if_statement";
const BINARY: &str = "binary_expression";
const UNARY: &str = "unary_expression";
const FUNC_LITERAL: &str = "func_literal";
const COMMENT: &str = "comment";
const CALLS: &[&str] = &["call_expression", "type_conversion_expression"];
const BRANCHES_THAT_LEAVE_A_SWITCH: &[&str] =
    &["break_statement", "continue_statement", "fallthrough_statement"];
const SIDE_EFFECT_OPERATORS: &[&str] = &["&", "<-"];
const EQUALS: &str = "==";
const OR: &str = "||";
const INITIALIZER_FIELD: &str = "initializer";
const CONDITION_FIELD: &str = "condition";
const CONSEQUENCE_FIELD: &str = "consequence";
const ALTERNATIVE_FIELD: &str = "alternative";
const LEFT_FIELD: &str = "left";
const RIGHT_FIELD: &str = "right";
const OPERATOR_FIELD: &str = "operator";
const MINIMUM_CHAIN: usize = 2;

// QF1003 de staticcheck: una cadena de `if`/`else if` que compara siempre lo mismo con `==`.
pub fn tagged_switches<'tree>(root: Node<'tree>, source: &[u8]) -> Vec<(Node<'tree>, String)> {
    descendants(root)
        .into_iter()
        .filter(|node| node.kind() == IF_STATEMENT)
        .filter(|node| node.parent().is_none_or(|parent| parent.kind() != IF_STATEMENT))
        .filter_map(|statement| {
            let tag = switch_tag(statement, source)?;
            Some((statement, collapsed_text(tag, source)))
        })
        .collect()
}

fn switch_tag<'tree>(first: Node<'tree>, source: &[u8]) -> Option<Node<'tree>> {
    let mut tag: Option<Node<'tree>> = None;
    let mut chain = 0_usize;
    let mut item = Some(first);
    while let Some(statement) = item {
        if statement.child_by_field_name(INITIALIZER_FIELD).is_some() || leaves_a_switch(statement) {
            return None;
        }
        let mut pairs = Vec::new();
        if !switch_pairs(
            statement.child_by_field_name(CONDITION_FIELD)?,
            source,
            &mut pairs,
        ) {
            return None;
        }
        let compared = *pairs.first()?;
        if tag.is_some_and(|known| !same_expression(known, compared, source)) {
            return None;
        }
        tag = Some(compared);
        chain = chain.saturating_add(1);
        item = statement
            .child_by_field_name(ALTERNATIVE_FIELD)
            .filter(|alternative| alternative.kind() == IF_STATEMENT);
    }
    tag.filter(|_| chain >= MINIMUM_CHAIN)
}

fn leaves_a_switch(statement: Node<'_>) -> bool {
    statement
        .child_by_field_name(CONSEQUENCE_FIELD)
        .is_some_and(|body| {
            descendants(body)
                .iter()
                .any(|node| BRANCHES_THAT_LEAVE_A_SWITCH.contains(&node.kind()))
        })
}

// `findSwitchPairs`: `x == a || x == b`, con el mismo `x` a la izquierda y nada con efectos.
fn switch_pairs<'tree>(expression: Node<'tree>, source: &[u8], pairs: &mut Vec<Node<'tree>>) -> bool {
    let expression = without_parentheses(expression);
    if expression.kind() != BINARY {
        return false;
    }
    let (Some(left), Some(right), Some(operator)) = (
        expression.child_by_field_name(LEFT_FIELD),
        expression.child_by_field_name(RIGHT_FIELD),
        expression.child_by_field_name(OPERATOR_FIELD),
    ) else {
        return false;
    };
    match operator.kind() {
        EQUALS => {
            if may_have_side_effects(left, source) || may_have_side_effects(right, source) {
                return false;
            }
            if pairs
                .first()
                .is_some_and(|first| !same_expression(*first, left, source))
            {
                return false;
            }
            pairs.push(left);
            true
        }
        OR => switch_pairs(left, source, pairs) && switch_pairs(right, source, pairs),
        _ => false,
    }
}

// `code.MayHaveSideEffects` sin la tabla de funciones puras: toda llamada cuenta, y `&` y `<-` también.
fn may_have_side_effects(expression: Node<'_>, source: &[u8]) -> bool {
    let mut pending = vec![expression];
    while let Some(node) = pending.pop() {
        if node.kind() == FUNC_LITERAL {
            continue;
        }
        if CALLS.contains(&node.kind()) {
            return true;
        }
        if node.kind() == UNARY
            && node
                .child_by_field_name(OPERATOR_FIELD)
                .and_then(|operator| operator.utf8_text(source).ok())
                .is_some_and(|operator| SIDE_EFFECT_OPERATORS.contains(&operator))
        {
            return true;
        }
        pending.extend(named_children(node));
    }
    false
}

// `astutil.Equal`: la misma forma, token por token; las funciones literales nunca son iguales.
fn same_expression(first: Node<'_>, second: Node<'_>, source: &[u8]) -> bool {
    let has_function = |node: Node<'_>| descendants(node).iter().any(|inner| inner.kind() == FUNC_LITERAL);
    !has_function(first) && !has_function(second) && tokens(first, source) == tokens(second, source)
}

fn tokens<'source>(node: Node<'_>, source: &'source [u8]) -> Vec<&'source str> {
    descendants(node)
        .into_iter()
        .filter(|inner| inner.child_count() == 0 && inner.kind() != COMMENT)
        .filter_map(|leaf| leaf.utf8_text(source).ok())
        .collect()
}

fn collapsed_text(node: Node<'_>, source: &[u8]) -> String {
    tokens(node, source).concat()
}
