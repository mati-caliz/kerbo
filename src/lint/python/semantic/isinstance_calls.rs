use tree_sitter::Node;

use super::builtin_calls::arguments;
use super::effects::contains_effect;
use super::model::Model;
use super::qualified::is_builtin_named;
use crate::fix::{Fix, TextEdit, TextPoint};
use crate::lint::python::Report;
use crate::lint::python::ast::{
    ATTRIBUTE, ATTRIBUTE_FIELD, CALL, COMMENT, FUNCTION_FIELD, IDENTIFIER, LEFT_FIELD, OBJECT_FIELD,
    RIGHT_FIELD, TUPLE, named, text, unparenthesized,
};
use crate::lint::python::branches::equivalent;
use crate::lint::python::codegen;
use crate::rules::PythonRule;

pub const BOOLEAN_OPERATOR: &str = "boolean_operator";
const OPERATOR_FIELD: &str = "operator";
const OR: &str = "or";
const OR_SEPARATOR: &str = " or ";
const ISINSTANCE: &str = "isinstance";
const AFFIX_METHODS: [&str; 2] = ["startswith", "endswith"];
const LIST_SPLAT: &str = "list_splat";
const STARRED_PREFIX: char = '*';

// SIM101: `isinstance(x, A) or isinstance(x, B)`, que se escribe `isinstance(x, (A, B))`. Como ruff,
// junta sólo las llamadas seguidas sobre el mismo valor y avisa una vez por grupo en el `or` entero.
pub fn duplicate_isinstance_reports(model: &Model<'_>, source: &[u8], operation: Node<'_>) -> Vec<Report> {
    if !is_or(operation, source) || is_inner_operand(operation, source) {
        return Vec::new();
    }
    let mut operands = Vec::new();
    flatten(operation, source, &mut operands);
    let mut reports = repeated_affix_checks(&operands, source);
    let mut groups: Vec<Vec<(Node<'_>, Node<'_>)>> = Vec::new();
    let mut previous: Option<Node<'_>> = None;
    for operand in &operands {
        let target = isinstance_target(model, source, *operand);
        let continues = matches!(
            (previous, target),
            (Some(last), Some(current)) if equivalent(last, current, source)
        );
        match (continues, target, groups.last_mut()) {
            (true, Some(target), Some(group)) => group.push((*operand, target)),
            (_, Some(target), _) => groups.push(vec![(*operand, target)]),
            _ => {}
        }
        previous = target;
    }
    for group in groups.iter().filter(|group| group.len() > 1) {
        let fix = merged_isinstance_fix(model, source, group);
        reports.push(Report::at(PythonRule::SIM101, operation, "").with_fix(fix));
    }
    reports
}

// El arreglo de SIM101: las llamadas seguidas (con sus paréntesis) pasan a una sola con los tipos
// juntos, abriendo las tuplas; inseguro, y sin arreglo si el valor tiene efectos.
fn merged_isinstance_fix(model: &Model<'_>, source: &[u8], group: &[(Node<'_>, Node<'_>)]) -> Option<Fix> {
    let (first, target) = group.first()?;
    let (last, _) = group.last()?;
    let is_builtin = |name: Node<'_>| is_builtin_named(model, name, source, text(name, source));
    if contains_effect(*target, source, &is_builtin) {
        return None;
    }
    let types: Vec<&str> = group
        .iter()
        .filter_map(|(operand, _)| arguments(unparenthesized(*operand)).positional.get(1).copied())
        .flat_map(|types| {
            let inner = unparenthesized(types);
            if inner.kind() == TUPLE {
                significant(inner)
                    .into_iter()
                    .map(|element| text(element, source))
                    .collect()
            } else {
                vec![text(types, source)]
            }
        })
        .collect();
    let mut listed = types.join(", ");
    if let [single] = types.as_slice()
        && single.starts_with(STARRED_PREFIX)
    {
        listed.push(',');
    }
    let combined = format!("{ISINSTANCE}({}, ({listed}))", text(*target, source));
    let fix = Fix::from_edits(
        source,
        vec![TextEdit {
            start: TextPoint::from_point(first.start_position()),
            end: TextPoint::from_point(last.end_position()),
            replacement: padded(combined, first.start_byte(), last.end_byte(), source),
        }],
    );
    Some(fix.marked_unsafe())
}

// El `pad` de ruff sobre un rango: un espacio si el reemplazo queda pegado a una letra.
fn padded(mut content: String, start: usize, end: usize, source: &[u8]) -> String {
    if source.get(end).is_some_and(u8::is_ascii_alphabetic) {
        content.push(' ');
    }
    if start
        .checked_sub(1)
        .and_then(|before| source.get(before))
        .is_some_and(u8::is_ascii_alphabetic)
    {
        content.insert(0, ' ');
    }
    content
}

// PIE810: `s.startswith("a") or s.startswith("b")`, que es `s.startswith(("a", "b"))`. Avisa una vez
// por variable y método en el `or` entero.
fn repeated_affix_checks(operands: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    let keys: Vec<Option<(&str, &str)>> = operands
        .iter()
        .map(|operand| affix_check(unparenthesized(*operand), source))
        .collect();
    let mut reported: Vec<(&str, &str)> = Vec::new();
    let mut reports = Vec::new();
    for key in keys.iter().flatten() {
        if !reported.contains(key) && keys.iter().flatten().filter(|other| *other == key).count() > 1 {
            reported.push(*key);
            if let Some(operation) = operands.first().and_then(|first| outermost_or(*first, source)) {
                let entries: Vec<(Node<'_>, Option<(&str, &str)>)> =
                    operands.iter().copied().zip(keys.iter().copied()).collect();
                let fix = merged_affix_fix(operation, &entries, *key, source);
                reports.push(Report::at(PythonRule::PIE810, operation, "").with_fix(Some(fix)));
            }
        }
    }
    reports
}

// El arreglo de PIE810: el `or` entero vuelto a escribir con el generador, con las llamadas del grupo
// juntas en la primera y sus argumentos en una tupla; inseguro, como en ruff.
fn merged_affix_fix(
    operation: Node<'_>,
    entries: &[(Node<'_>, Option<(&str, &str)>)],
    key: (&str, &str),
    source: &[u8],
) -> Fix {
    let operands: Vec<Node<'_>> = entries.iter().map(|(operand, _)| *operand).collect();
    let in_group: Vec<bool> = entries.iter().map(|(_, other)| *other == Some(key)).collect();
    let words: Vec<String> = operands
        .iter()
        .zip(&in_group)
        .filter(|(_, grouped)| **grouped)
        .filter_map(|(operand, _)| arguments(unparenthesized(*operand)).positional.first().copied())
        .flat_map(|word| {
            let inner = unparenthesized(word);
            if inner.kind() == TUPLE {
                significant(inner)
                    .into_iter()
                    .map(|element| codegen::unparse(element, codegen::COMMA, source))
                    .collect()
            } else {
                vec![codegen::unparse(word, codegen::COMMA, source)]
            }
        })
        .collect();
    let (receiver, method) = key;
    let tuple = if let [single] = words.as_slice() {
        format!("({single},)")
    } else {
        format!("({})", words.join(", "))
    };
    let mut merged = Some(format!("{receiver}.{method}({tuple})"));
    let values: Vec<String> = operands
        .iter()
        .zip(&in_group)
        .filter_map(|(operand, grouped)| {
            if *grouped {
                merged.take()
            } else {
                Some(codegen::unparse(*operand, codegen::OR.saturating_add(1), source))
            }
        })
        .collect();
    Fix::replace(operation, &values.join(OR_SEPARATOR), source).marked_unsafe()
}

fn significant(node: Node<'_>) -> Vec<Node<'_>> {
    named(node)
        .into_iter()
        .filter(|element| element.kind() != COMMENT)
        .collect()
}

fn outermost_or<'tree>(operand: Node<'tree>, source: &[u8]) -> Option<Node<'tree>> {
    std::iter::successors(operand.parent(), Node::parent)
        .take_while(|ancestor| is_or(*ancestor, source))
        .last()
}

fn affix_check<'source>(call: Node<'_>, source: &'source [u8]) -> Option<(&'source str, &'source str)> {
    let function = call
        .child_by_field_name(FUNCTION_FIELD)
        .filter(|function| call.kind() == CALL && function.kind() == ATTRIBUTE)?;
    let method = text(function.child_by_field_name(ATTRIBUTE_FIELD)?, source);
    let receiver = function
        .child_by_field_name(OBJECT_FIELD)
        .filter(|receiver| receiver.kind() == IDENTIFIER)?;
    let given = arguments(call);
    let single = matches!(given.positional.as_slice(), [only] if only.kind() != LIST_SPLAT)
        && given.keywords.is_empty();
    (AFFIX_METHODS.contains(&method) && single).then(|| (text(receiver, source), method))
}

fn is_or(operation: Node<'_>, source: &[u8]) -> bool {
    operation.kind() == BOOLEAN_OPERATOR
        && operation
            .child_by_field_name(OPERATOR_FIELD)
            .is_some_and(|operator| text(operator, source) == OR)
}

// Un `a or b` sin paréntesis adentro de otro `or` es parte del mismo `BoolOp` de ruff.
fn is_inner_operand(operation: Node<'_>, source: &[u8]) -> bool {
    operation.parent().is_some_and(|parent| is_or(parent, source))
}

fn flatten<'tree>(node: Node<'tree>, source: &[u8], operands: &mut Vec<Node<'tree>>) {
    if is_or(node, source) {
        for side in [LEFT_FIELD, RIGHT_FIELD] {
            if let Some(child) = node.child_by_field_name(side) {
                flatten(child, source, operands);
            }
        }
    } else {
        operands.push(node);
    }
}

fn isinstance_target<'tree>(model: &Model<'_>, source: &[u8], operand: Node<'tree>) -> Option<Node<'tree>> {
    let call = unparenthesized(operand);
    if call.kind() != CALL
        || !call
            .child_by_field_name(FUNCTION_FIELD)
            .is_some_and(|function| is_builtin_named(model, function, source, ISINSTANCE))
    {
        return None;
    }
    let given = arguments(call);
    match (given.positional.as_slice(), given.keywords.is_empty()) {
        ([target, _], true) => Some(*target),
        _ => None,
    }
}
