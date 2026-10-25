use tree_sitter::Node;

use crate::fix::{Fix, TextEdit};
use crate::lint::python::ast::{
    ARGUMENTS_FIELD, COMMENT, INTERPOLATION, LEFT_FIELD, RIGHT_FIELD, TUPLE, named, text, unparenthesized,
};
use crate::lint::python::codegen::unparse;
use crate::lint::python::fix_edits::point_of;

const GENERATOR: &str = "generator_expression";
const FOR_IN_CLAUSE: &str = "for_in_clause";
const IF_CLAUSE: &str = "if_clause";
const BODY_FIELD: &str = "body";
const ASYNC: &str = "async";
const COMMA: u8 = b',';
const OPEN_BRACE: u8 = b'{';
const CLOSE_BRACE: u8 = b'}';
const CLOSE_PAREN: u8 = b')';
const LOWEST: u8 = 0;
const LIST: &str = "list";

// El argumento único de `f(...)` como lo ve ruff: un generador sin paréntesis propios (`f(x for x in
// y)`) va sin los de la llamada.
struct Single<'tree> {
    call: Node<'tree>,
    arguments: Node<'tree>,
    node: Node<'tree>,
    start: usize,
    end: usize,
    parenthesized_generator: bool,
}

fn single<'tree>(call: Node<'tree>, argument: Node<'tree>) -> Option<Single<'tree>> {
    let arguments = call.child_by_field_name(ARGUMENTS_FIELD)?;
    if arguments.kind() == GENERATOR {
        return Some(Single {
            call,
            arguments,
            node: arguments,
            start: arguments.start_byte().saturating_add(1),
            end: arguments.end_byte().saturating_sub(1),
            parenthesized_generator: false,
        });
    }
    let node = unparenthesized(argument);
    Some(Single {
        call,
        arguments,
        node,
        start: node.start_byte(),
        end: node.end_byte(),
        parenthesized_generator: node.kind() == GENERATOR,
    })
}

fn edit(start: usize, end: usize, replacement: &str, source: &[u8]) -> TextEdit {
    TextEdit {
        start: point_of(source, start),
        end: point_of(source, end),
        replacement: replacement.to_owned(),
    }
}

fn unsafe_fix(edits: Vec<TextEdit>, source: &[u8]) -> Fix {
    Fix::from_edits(source, edits).marked_unsafe()
}

// Donde va el corchete de cierre: en la coma de más después del argumento, o en el `)`.
fn closing_start(single: &Single<'_>, source: &[u8]) -> usize {
    let call_end = single.call.end_byte();
    let closing = single.arguments.end_byte().saturating_sub(1);
    source
        .get(single.end..call_end)
        .and_then(|rest| rest.iter().position(|byte| *byte == COMMA))
        .map_or(closing, |comma| single.end.saturating_add(comma))
}

// El `pad_start` y `pad_end` de C4: adentro de un f-string, un espacio para no pegar `{{` o `}}`.
fn in_interpolation(node: Node<'_>) -> bool {
    std::iter::successors(node.parent(), Node::parent).any(|ancestor| ancestor.kind() == INTERPOLATION)
}

fn padded_open(open: &str, call: Node<'_>, source: &[u8]) -> String {
    let before = call
        .start_byte()
        .checked_sub(1)
        .and_then(|index| source.get(index));
    if in_interpolation(call) && before == Some(&OPEN_BRACE) {
        format!(" {open}")
    } else {
        open.to_owned()
    }
}

fn padded_close(close: &str, call: Node<'_>, source: &[u8]) -> String {
    if in_interpolation(call) && source.get(call.end_byte()) == Some(&CLOSE_BRACE) {
        format!("{close} ")
    } else {
        close.to_owned()
    }
}

// C400 y C401: `list(f(x) for x in y)` pasa a `[f(x) for x in y]`, y `list(x for x in y)` a
// `list(y)`.
pub fn generator_fix(call: Node<'_>, argument: Node<'_>, builtin: &str, source: &[u8]) -> Option<Fix> {
    let single = single(call, argument)?;
    let generator = single.node;
    if let Some(iterable) = identity_iterable(generator, source) {
        let replacement = format!("{builtin}({})", text(iterable, source));
        return Some(unsafe_fix(
            vec![edit(call.start_byte(), call.end_byte(), &replacement, source)],
            source,
        ));
    }
    let (open, close) = if builtin == LIST {
        (String::from("["), String::from("]"))
    } else {
        (padded_open("{", call, source), padded_close("}", call, source))
    };
    let mut edits = vec![
        edit(
            call.start_byte(),
            single.arguments.start_byte().saturating_add(1),
            &open,
            source,
        ),
        edit(closing_start(&single, source), call.end_byte(), &close, source),
    ];
    if single.parenthesized_generator {
        let printed = unparse(generator, LOWEST, source);
        let inner = printed
            .get(1..printed.len().saturating_sub(1))
            .unwrap_or_default();
        let outer = std::iter::successors(Some(generator), Node::parent)
            .take_while(|node| node.id() != single.arguments.id())
            .last()
            .unwrap_or(generator);
        edits.push(edit(outer.start_byte(), outer.end_byte(), inner, source));
    }
    Some(unsafe_fix(edits, source))
}

// `x for x in y`, con una sola cláusula, sin `if` ni `async` y el elemento igual al destino.
fn identity_iterable<'tree>(generator: Node<'tree>, source: &[u8]) -> Option<Node<'tree>> {
    let clauses: Vec<Node<'tree>> = named(generator)
        .into_iter()
        .filter(|child| matches!(child.kind(), FOR_IN_CLAUSE | IF_CLAUSE))
        .collect();
    let [clause] = clauses.as_slice() else {
        return None;
    };
    if clause.kind() != FOR_IN_CLAUSE || text(*clause, source).starts_with(ASYNC) {
        return None;
    }
    let element = generator.child_by_field_name(BODY_FIELD)?;
    let target = clause.child_by_field_name(LEFT_FIELD)?;
    (unparse(element, LOWEST, source) == unparse(target, LOWEST, source))
        .then(|| clause.child_by_field_name(RIGHT_FIELD))
        .flatten()
}

// C403: `set([x for x in y])` pasa a `{x for x in y}`.
pub fn list_comprehension_set_fix(call: Node<'_>, argument: Node<'_>, source: &[u8]) -> Option<Fix> {
    let single = single(call, argument)?;
    let inner = String::from_utf8_lossy(
        source
            .get(single.start.saturating_add(1)..single.end.saturating_sub(1))
            .unwrap_or_default(),
    )
    .into_owned();
    let outer = argument;
    Some(unsafe_fix(
        vec![
            edit(
                call.start_byte(),
                single.arguments.start_byte().saturating_add(1),
                &padded_open("{", call, source),
                source,
            ),
            edit(
                closing_start(&single, source),
                call.end_byte(),
                &padded_close("}", call, source),
                source,
            ),
            edit(outer.start_byte(), outer.end_byte(), &inner, source),
        ],
        source,
    ))
}

fn elements(node: Node<'_>) -> Vec<Node<'_>> {
    named(node)
        .into_iter()
        .filter(|child| child.kind() != COMMENT)
        .collect()
}

// C405: `set([1, 2])` pasa a `{1, 2}`, `set(())` a `set()` y `set((1,))` a `{1}`.
pub fn literal_set_fix(call: Node<'_>, argument: Node<'_>, source: &[u8]) -> Option<Fix> {
    let single = single(call, argument)?;
    let literal = single.node;
    let items = elements(literal);
    let edits = match items.as_slice() {
        [] => vec![edit(call.start_byte(), call.end_byte(), "set()", source)],
        [only]
            if literal.kind() == TUPLE
                && only.start_byte() == literal.start_byte().saturating_add(1)
                && only.end_byte().saturating_add(2) == literal.end_byte() =>
        {
            vec![
                edit(
                    call.start_byte(),
                    only.start_byte(),
                    &padded_open("{", call, source),
                    source,
                ),
                edit(
                    only.end_byte(),
                    call.end_byte(),
                    &padded_close("}", call, source),
                    source,
                ),
            ]
        }
        _ => vec![
            edit(
                call.start_byte(),
                literal.start_byte().saturating_add(1),
                &padded_open("{", call, source),
                source,
            ),
            edit(
                literal.end_byte().saturating_sub(1),
                call.end_byte(),
                &padded_close("}", call, source),
                source,
            ),
        ],
    };
    Some(unsafe_fix(edits, source))
}

// C409: `tuple([1, 2])` pasa a `(1, 2)`, con la coma si queda un solo elemento.
pub fn literal_tuple_fix(call: Node<'_>, argument: Node<'_>, source: &[u8]) -> Fix {
    let literal = unparenthesized(argument);
    let items = elements(literal);
    let needs_comma = match items.as_slice() {
        [only] => !source
            .get(only.end_byte()..literal.end_byte())
            .unwrap_or_default()
            .contains(&COMMA),
        _ => false,
    };
    let close = if needs_comma { ",)" } else { ")" };
    unsafe_fix(
        vec![
            edit(
                call.start_byte(),
                literal.start_byte().saturating_add(1),
                "(",
                source,
            ),
            edit(
                literal.end_byte().saturating_sub(1),
                call.end_byte(),
                close,
                source,
            ),
        ],
        source,
    )
}

// C410: `list([1, 2])` pasa a `[1, 2]`, y `list((1, 2))` también.
pub fn literal_list_fix(call: Node<'_>, argument: Node<'_>, source: &[u8]) -> Fix {
    let literal = unparenthesized(argument);
    let mut edits = vec![
        edit(call.start_byte(), literal.start_byte(), "", source),
        edit(literal.end_byte(), call.end_byte(), "", source),
    ];
    if literal.kind() == TUPLE {
        edits.push(edit(
            literal.start_byte(),
            literal.start_byte().saturating_add(1),
            "[",
            source,
        ));
        edits.push(edit(
            literal.end_byte().saturating_sub(1),
            literal.end_byte(),
            "]",
            source,
        ));
    }
    unsafe_fix(edits, source)
}

// C418: `dict({...})` pasa a `{...}`.
pub fn literal_dict_fix(call: Node<'_>, argument: Node<'_>, source: &[u8]) -> Fix {
    let literal = unparenthesized(argument);
    unsafe_fix(
        vec![
            edit(call.start_byte(), literal.start_byte(), "", source),
            edit(literal.end_byte(), call.end_byte(), "", source),
        ],
        source,
    )
}

mod calls;
mod collections;

pub use calls::{
    comprehension_argument_fix, comprehension_in_call_fix, dict_comprehension_fix, list_call_fix,
};
pub use collections::{call_around_sorted_fix, collection_call_fix, double_cast_fix, literal_dict_pairs_fix};
