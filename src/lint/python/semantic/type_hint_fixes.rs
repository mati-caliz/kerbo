use tree_sitter::Node;

use super::importer::{ImportRequest, ImportStyle, get_or_import_member};
use super::type_hints::{Hints, subscript_parts, unwrapped};
use crate::fix::{Fix, TextEdit, TextPoint};
use crate::lint::python::ast::{RIGHT_FIELD, named, text};
use crate::lint::python::branches::equivalent;
use crate::lint::python::codegen;
use crate::lint::python::fix_edits::{Parentheses, commented_unsafe, remove_argument};

const ELEMENT_SEPARATOR: &str = ", ";
const ASYNC_PREFIX: &str = "Async";
const ITERATOR: &str = "Iterator";
const ASYNC_ITERATOR: &str = "AsyncIterator";
const UNION_SEPARATOR: &str = " | ";
const LITERAL: &str = "Literal";
const UNION: &str = "Union";
const NONE_LITERAL: &str = "none";
const NONE_WORD: &str = "None";
const UNION_TYPE: &str = "union_type";
const BINARY_OPERATOR: &str = "binary_operator";
const OPERATOR_FIELD: &str = "operator";
const BIT_OR: &str = "|";
const WRAPPERS: [&str; 2] = ["type", "parenthesized_expression"];
const STATEMENT_PARENTS: [&str; 2] = ["module", "block"];

// PYI064: `x: Final[Literal[1]] = 1` pasa a `x: Final = 1`, y sin valor `x: Final = 1`; si el valor es
// otro, ruff no lo arregla.
pub fn final_literal_fix(
    final_subscript: Node<'_>,
    literal: Node<'_>,
    assignment: Node<'_>,
    source: &[u8],
) -> Option<Fix> {
    let (head, _) = subscript_parts(final_subscript)?;
    let mut edits = vec![TextEdit {
        start: TextPoint::from_point(head.end_position()),
        end: TextPoint::from_point(final_subscript.end_position()),
        replacement: String::new(),
    }];
    match assignment.child_by_field_name(RIGHT_FIELD) {
        Some(value) if equivalent(unwrapped(value), unwrapped(literal), source) => {}
        Some(_) => return None,
        None => edits.push(TextEdit {
            start: TextPoint::from_point(final_subscript.end_position()),
            end: TextPoint::from_point(final_subscript.end_position()),
            replacement: format!(" = {}", text(literal, source)),
        }),
    }
    Some(Fix::from_edits(source, edits))
}

// PYI062: el `Literal` vuelto a escribir con cada valor una vez, abriendo los `Literal` de adentro.
pub fn literal_dedup_fix(literal: Node<'_>, members: &[Node<'_>], source: &[u8]) -> Option<Fix> {
    let mut unique: Vec<Node<'_>> = Vec::new();
    for member in members {
        if !unique.iter().any(|kept| equivalent(*kept, *member, source)) {
            unique.push(*member);
        }
    }
    rewritten_literal(literal, &unique, source)
}

// RUF041: el `Literal` con los `Literal` de adentro abiertos.
pub fn nested_literal_fix(literal: Node<'_>, members: &[Node<'_>], source: &[u8]) -> Option<Fix> {
    rewritten_literal(literal, members, source)
}

// PYI061: `Literal[None]` pasa a `None` y `Literal[1, None]` a `Literal[1] | None`. Sin arreglo si ya
// está adentro de un `|` que tiene un `None`; inseguro con comentarios (y fuera de un stub, que lo
// decide quien llama).
pub fn none_literal_fix(literal: Node<'_>, members: &[Node<'_>], source: &[u8]) -> Option<Fix> {
    let (head, _) = subscript_parts(literal)?;
    let others: Vec<Node<'_>> = members
        .iter()
        .copied()
        .filter(|member| member.kind() != NONE_LITERAL)
        .collect();
    if inside_union_with_none(literal, source) {
        return None;
    }
    let replacement = if others.is_empty() {
        String::from(NONE_WORD)
    } else {
        let listed: Vec<String> = others
            .iter()
            .map(|member| codegen::unparse(*member, codegen::COMMA, source))
            .collect();
        format!(
            "{}[{}] | {NONE_WORD}",
            codegen::unparse(head, codegen::MAX, source),
            listed.join(ELEMENT_SEPARATOR)
        )
    };
    let fix = commented_unsafe(Fix::replace(literal, &replacement, source), literal, source)
        .isolated(enclosing_statement(literal).map(|statement| statement.id()));
    Some(fix)
}

fn inside_union_with_none(literal: Node<'_>, source: &[u8]) -> bool {
    let root = std::iter::successors(literal.parent(), Node::parent)
        .take_while(|ancestor| is_bit_or(*ancestor, source) || WRAPPERS.contains(&ancestor.kind()))
        .last();
    let Some(root) = root else {
        return false;
    };
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if is_bit_or(node, source) || WRAPPERS.contains(&node.kind()) {
            pending.extend(named(node));
        } else if node.kind() == NONE_LITERAL {
            return true;
        }
    }
    false
}

fn is_bit_or(node: Node<'_>, source: &[u8]) -> bool {
    node.kind() == UNION_TYPE
        || (node.kind() == BINARY_OPERATOR
            && node
                .child_by_field_name(OPERATOR_FIELD)
                .is_some_and(|operator| text(operator, source) == BIT_OR))
}

fn enclosing_statement(node: Node<'_>) -> Option<Node<'_>> {
    std::iter::successors(Some(node), Node::parent).find(|ancestor| {
        ancestor
            .parent()
            .is_some_and(|parent| STATEMENT_PARENTS.contains(&parent.kind()))
    })
}

// PYI030: los `Literal` de una unión juntos en el primero; el resto sigue en la unión, con `Union[...]`
// si la unión venía así o con `|` si no. Inseguro con comentarios.
pub fn literal_union_fix(hints: Hints<'_, '_, '_>, union: Node<'_>, members: &[Node<'_>]) -> Option<Fix> {
    let source = hints.source;
    let (literals, others): (Vec<Node<'_>>, Vec<Node<'_>>) = members
        .iter()
        .partition(|member| hints.is_typing_subscript(**member, LITERAL));
    let (head, _) = subscript_parts(*literals.first()?)?;
    let mut values = Vec::new();
    for literal in &literals {
        hints.literal_members(*literal, &mut values);
    }
    let listed: Vec<String> = values
        .iter()
        .map(|value| codegen::unparse(*value, codegen::COMMA, source))
        .collect();
    let merged = format!(
        "{}[{}]",
        codegen::unparse(head, codegen::MAX, source),
        listed.join(ELEMENT_SEPARATOR)
    );
    let replacement = if others.is_empty() {
        merged
    } else if hints.is_typing_subscript(union, UNION) {
        let (union_head, _) = subscript_parts(union)?;
        let rest = others
            .iter()
            .map(|other| codegen::unparse(*other, codegen::COMMA, source));
        let elements: Vec<String> = std::iter::once(merged).chain(rest).collect();
        format!(
            "{}[{}]",
            codegen::unparse(union_head, codegen::MAX, source),
            elements.join(ELEMENT_SEPARATOR)
        )
    } else {
        let rest = others
            .iter()
            .map(|other| codegen::unparse(*other, codegen::BIT_OR.saturating_add(1), source));
        std::iter::once(merged)
            .chain(rest)
            .collect::<Vec<String>>()
            .join(UNION_SEPARATOR)
    };
    Some(commented_unsafe(
        Fix::replace(union, &replacement, source),
        union,
        source,
    ))
}

fn rewritten_literal(literal: Node<'_>, unique: &[Node<'_>], source: &[u8]) -> Option<Fix> {
    let (head, _) = subscript_parts(literal)?;
    let listed: Vec<String> = unique
        .iter()
        .map(|member| codegen::unparse(*member, codegen::COMMA, source))
        .collect();
    let replacement = format!(
        "{}[{}]",
        codegen::unparse(head, codegen::MAX, source),
        listed.join(ELEMENT_SEPARATOR)
    );
    Some(commented_unsafe(
        Fix::replace(literal, &replacement, source),
        literal,
        source,
    ))
}

// PYI059: `Generic[...]` pasa al final de las bases; inseguro, como en ruff.
pub fn generic_last_fix(bases: Node<'_>, generic: Node<'_>, last: Node<'_>, source: &[u8]) -> Fix {
    let mut edits = remove_argument(generic, bases, Parentheses::Preserve, source).edits;
    edits.push(TextEdit {
        start: TextPoint::from_point(last.end_position()),
        end: TextPoint::from_point(last.end_position()),
        replacement: format!("{ELEMENT_SEPARATOR}{}", text(generic, source)),
    });
    Fix::from_edits(source, edits).marked_unsafe()
}

// El arreglo de PYI058: `Generator[X, None, None]` pasa a `Iterator[X]` del mismo módulo, con el import
// que haga falta.
pub fn iterator_fix(
    hints: Hints<'_, '_, '_>,
    head: Node<'_>,
    qualified: &str,
    slices: &[Node<'_>],
) -> Option<Fix> {
    let (module, generator) = qualified.rsplit_once('.')?;
    let iterator = if generator.starts_with(ASYNC_PREFIX) {
        ASYNC_ITERATOR
    } else {
        ITERATOR
    };
    let request = ImportRequest {
        module,
        member: iterator,
        style: ImportStyle::ImportFrom,
    };
    let (binding, import) = get_or_import_member(hints.model, hints.source, head, request)?;
    let mut edits = vec![TextEdit {
        start: TextPoint::from_point(head.start_position()),
        end: TextPoint::from_point(head.end_position()),
        replacement: binding,
    }];
    if let (Some(first), Some(last)) = (slices.first(), slices.last()) {
        edits.push(TextEdit {
            start: TextPoint::from_point(first.start_position()),
            end: TextPoint::from_point(last.end_position()),
            replacement: codegen::unparse(*first, codegen::COMMA, hints.source),
        });
    }
    edits.push(import);
    Some(Fix::from_edits(hints.source, edits))
}
