use tree_sitter::Node;

use super::comprehension_fixes::{
    comprehension_argument_fix, comprehension_in_call_fix, generator_fix, literal_list_fix, literal_tuple_fix,
};
use super::model::Model;
use super::qualified::is_builtin_named;
use crate::lint::python::Report;
use crate::lint::python::ast::{
    ARGUMENTS_FIELD, BODY_FIELD, CALL, COMMENT, FUNCTION_FIELD, IDENTIFIER, KEYWORD_ARGUMENT, LIST, TUPLE,
    named, text, unparenthesized,
};
use crate::rules::PythonRule;

const LIST_BUILTIN: &str = "list";
const TUPLE_BUILTIN: &str = "tuple";
const DICT_BUILTIN: &str = "dict";
const PAIR_LENGTH: usize = 2;
const SORTED: &str = "sorted";
const AROUND_SORTED: [&str; 2] = ["list", "reversed"];
const SHORT_CIRCUITING: [&str; 2] = ["any", "all"];
const GENERATOR: &str = "generator_expression";
const ARGUMENT_LIST: &str = "argument_list";
const DICTIONARY_SPLAT: &str = "dictionary_splat";
const COMPREHENSIONS: [&str; 2] = ["list_comprehension", "set_comprehension"];
const FOR_IN: &str = "for_in_clause";
const ASYNC: &str = "async";
const AWAIT: &str = "await";

struct Arguments<'tree> {
    positional: Vec<Node<'tree>>,
    has_keywords: bool,
}

// C400 (`list(x for x in y)`), C409 (`tuple([1, 2])`), C413 (`list(sorted(x))`) y C419
// (`any([x for x in y])`).
pub fn collection_call_reports(model: &Model<'_>, source: &[u8], call: Node<'_>) -> Vec<Report> {
    let Some(function) = call.child_by_field_name(FUNCTION_FIELD) else {
        return Vec::new();
    };
    let arguments = arguments(call);
    let is_builtin = |name: &str| is_builtin_named(model, function, source, name);
    let named_function = (function.kind() == IDENTIFIER).then(|| text(function, source));
    let single = match arguments.positional.as_slice() {
        [single] if !arguments.has_keywords => Some(unparenthesized(*single)),
        _ => None,
    };
    let first = arguments.positional.first().copied().map(unparenthesized);
    let mut reports = Vec::new();
    if let Some(single) = single
        && named_function == Some(LIST_BUILTIN)
        && single.kind() == GENERATOR
        && is_builtin(LIST_BUILTIN)
    {
        let fix = arguments
            .positional
            .first()
            .and_then(|argument| generator_fix(call, *argument, LIST_BUILTIN, source));
        reports.push(Report::at(PythonRule::C400, call, "").with_fix(fix));
    }
    if let Some(single) = single
        && named_function == Some(TUPLE_BUILTIN)
        && matches!(single.kind(), TUPLE | LIST)
        && is_builtin(TUPLE_BUILTIN)
    {
        let fix = Some(literal_tuple_fix(call, single, source));
        reports.push(Report::at(PythonRule::C409, call, "").with_fix(fix));
    }
    if let Some(single) = single
        && named_function == Some(LIST_BUILTIN)
        && matches!(single.kind(), TUPLE | LIST)
        && is_builtin(LIST_BUILTIN)
    {
        let fix = Some(literal_list_fix(call, single, source));
        reports.push(Report::at(PythonRule::C410, call, "").with_fix(fix));
    }
    if let Some(single) = single
        && named_function == Some(DICT_BUILTIN)
        && matches!(single.kind(), TUPLE | LIST)
        && named(single)
            .into_iter()
            .filter(|element| element.kind() != COMMENT)
            .all(|element| element.kind() == TUPLE && named(element).len() == PAIR_LENGTH)
        && is_builtin(DICT_BUILTIN)
    {
        reports.push(Report::at(PythonRule::C406, call, ""));
    }
    if let Some(first) = first
        && AROUND_SORTED.iter().any(|name| is_builtin(name))
        && is_sorted_call(model, source, first)
    {
        reports.push(Report::at(PythonRule::C413, call, ""));
    }
    if let Some(first) = first
        && !arguments.has_keywords
        && SHORT_CIRCUITING.iter().any(|name| is_builtin(name))
        && COMPREHENSIONS.contains(&first.kind())
        && is_synchronous(first)
    {
        let fix = if arguments.positional.len() > 1 {
            Some(comprehension_in_call_fix(first, source))
        } else {
            comprehension_argument_fix(call, first, source)
        };
        reports.push(Report::at(PythonRule::C419, first, "").with_fix(fix));
    }
    reports
}

fn arguments(call: Node<'_>) -> Arguments<'_> {
    let Some(list) = call.child_by_field_name(ARGUMENTS_FIELD) else {
        return Arguments {
            positional: Vec::new(),
            has_keywords: false,
        };
    };
    if list.kind() != ARGUMENT_LIST {
        return Arguments {
            positional: vec![list],
            has_keywords: false,
        };
    }
    let (keywords, positional): (Vec<Node<'_>>, Vec<Node<'_>>) = named(list)
        .into_iter()
        .filter(|argument| argument.kind() != COMMENT)
        .partition(|argument| matches!(argument.kind(), KEYWORD_ARGUMENT | DICTIONARY_SPLAT));
    Arguments {
        positional,
        has_keywords: !keywords.is_empty(),
    }
}

fn is_sorted_call(model: &Model<'_>, source: &[u8], node: Node<'_>) -> bool {
    node.kind() == CALL
        && node
            .child_by_field_name(FUNCTION_FIELD)
            .is_some_and(|inner| is_builtin_named(model, inner, source, SORTED))
}

// Ni un `async for` en la comprensión ni un `await` en lo que produce.
fn is_synchronous(comprehension: Node<'_>) -> bool {
    let has_async_clause = named(comprehension)
        .into_iter()
        .filter(|clause| clause.kind() == FOR_IN)
        .any(|clause| {
            let mut cursor = clause.walk();
            clause.children(&mut cursor).any(|token| token.kind() == ASYNC)
        });
    let has_await = comprehension
        .child_by_field_name(BODY_FIELD)
        .is_some_and(contains_await);
    !has_async_clause && !has_await
}

fn contains_await(node: Node<'_>) -> bool {
    node.kind() == AWAIT || named(node).into_iter().any(contains_await)
}
