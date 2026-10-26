use tree_sitter::Node;

use super::builtin_calls::arguments;
use crate::fix::Fix;
use crate::lint::python::ast::{COMMENT, INTERPOLATION, LAMBDA, named, text, unparenthesized};

const PARAMETERS_FIELD: &str = "parameters";
const BODY_FIELD: &str = "body";
const CONDITIONAL_EXPRESSION: &str = "conditional_expression";
const TUPLE: &str = "tuple";
const LIST: &str = "list";
const LIST_SPLAT: &str = "list_splat";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Comprehension {
    Generator,
    List,
    Set,
    Dict,
}

// El `fix_unnecessary_map` de ruff con libcst: el cuerpo de la lambda tal como está, su parámetro
// como variable del `for` y el iterable entre paréntesis si es un `if` o una lambda sueltos.
pub fn map_comprehension_fix(
    source: &[u8],
    replaced: Node<'_>,
    map_call: Node<'_>,
    comprehension: Comprehension,
) -> Option<Fix> {
    let given = arguments(map_call);
    let [lambda, iterable] = given.positional.as_slice() else {
        return None;
    };
    let lambda = unparenthesized(*lambda);
    if lambda.kind() != LAMBDA {
        return None;
    }
    let target = match without_comments(lambda.child_by_field_name(PARAMETERS_FIELD)?).as_slice() {
        [parameter] => text(*parameter, source),
        _ => return None,
    };
    let body = lambda.child_by_field_name(BODY_FIELD)?;
    let iterable_text = text(*iterable, source);
    let iterable_text = if matches!(iterable.kind(), CONDITIONAL_EXPRESSION | LAMBDA) {
        format!("({iterable_text})")
    } else {
        iterable_text.to_owned()
    };
    let clause = format!("for {target} in {iterable_text}");
    let body_text = text(body, source);
    let content = match comprehension {
        Comprehension::Generator => format!("({body_text} {clause})"),
        Comprehension::List => format!("[{body_text} {clause}]"),
        Comprehension::Set => format!("{{{body_text} {clause}}}"),
        Comprehension::Dict => {
            let pair = unparenthesized(body);
            if !matches!(pair.kind(), TUPLE | LIST) {
                return None;
            }
            let elements = without_comments(pair);
            let [key, value] = elements.as_slice() else {
                return None;
            };
            if key.kind() == LIST_SPLAT || value.kind() == LIST_SPLAT {
                return None;
            }
            format!("{{{}: {} {clause}}}", text(*key, source), text(*value, source))
        }
    };
    let in_interpolation = replaced
        .parent()
        .is_some_and(|parent| parent.kind() == INTERPOLATION);
    let content = if in_interpolation && matches!(comprehension, Comprehension::Set | Comprehension::Dict) {
        format!(" {content} ")
    } else {
        content
    };
    Some(Fix::replace(replaced, &content, source).marked_unsafe())
}

fn without_comments(node: Node<'_>) -> Vec<Node<'_>> {
    named(node)
        .into_iter()
        .filter(|child| child.kind() != COMMENT)
        .collect()
}
