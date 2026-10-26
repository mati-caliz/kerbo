use tree_sitter::Node;

use super::Report;
use super::ast::{ALIAS_FIELD, BLOCK, COMMENT, IDENTIFIER, VALUE_FIELD, named, text, unparenthesized};
use crate::fix::Fix;
use crate::rules::PythonRule;

const EXCEPT_CLAUSES: [&str; 2] = ["except_clause", "except_group_clause"];
const AS_PATTERN: &str = "as_pattern";
const RAISE: &str = "raise_statement";
const TRY: &str = "try_statement";
const FINALLY: &str = "finally_clause";
const CAUSE_FIELD: &str = "cause";
const RAISE_KEYWORD: &str = "raise";

// TRY201: `except E as error: raise error`, que se escribe `raise` a secas para no tocar el
// traceback. Avisa en el nombre.
pub fn reports(nodes: &[Node<'_>], source: &[u8]) -> Vec<Report> {
    nodes
        .iter()
        .filter(|node| EXCEPT_CLAUSES.contains(&node.kind()))
        .flat_map(|handler| verbose_raises(*handler, source))
        .collect()
}

fn verbose_raises(handler: Node<'_>, source: &[u8]) -> Vec<Report> {
    let Some(name) = exception_name(handler, source) else {
        return Vec::new();
    };
    let mut raises = Vec::new();
    for block in named(handler).into_iter().filter(|child| child.kind() == BLOCK) {
        collect_raises(block, &mut raises);
    }
    raises
        .into_iter()
        .filter(|raise| raise.child_by_field_name(CAUSE_FIELD).is_none())
        .filter_map(|raise| raised_value(raise).map(|value| (raise, value)))
        .filter(|(_, value)| value.kind() == IDENTIFIER && text(*value, source) == name)
        .map(|(raise, value)| {
            let fix = Fix::replace(raise, RAISE_KEYWORD, source).marked_unsafe();
            Report::at(PythonRule::TRY201, value, "").with_fix(Some(fix))
        })
        .collect()
}

fn exception_name<'source>(handler: Node<'_>, source: &'source [u8]) -> Option<&'source str> {
    let value = handler
        .child_by_field_name(VALUE_FIELD)
        .filter(|value| value.kind() == AS_PATTERN)?;
    let alias = value.child_by_field_name(ALIAS_FIELD)?;
    named(alias)
        .into_iter()
        .find(|inner| inner.kind() == IDENTIFIER)
        .map(|identifier| text(identifier, source))
}

// El `RaiseStatementVisitor` de ruff: todas las sentencias, también las de funciones anidadas. De un
// `try` anidado sólo mira adentro de las sentencias del cuerpo y del `finally`, no a ellas mismas
// (ruff las recorre con `walk_stmt`), así que un `raise` suelto ahí no cuenta.
fn collect_raises<'tree>(node: Node<'tree>, raises: &mut Vec<Node<'tree>>) {
    match node.kind() {
        RAISE => raises.push(node),
        TRY => {
            let blocks = named(node).into_iter().flat_map(|child| match child.kind() {
                BLOCK => vec![child],
                FINALLY => named(child)
                    .into_iter()
                    .filter(|inner| inner.kind() == BLOCK)
                    .collect(),
                _ => Vec::new(),
            });
            for statement in blocks.flat_map(named) {
                for child in named(statement) {
                    collect_raises(child, raises);
                }
            }
        }
        _ => {
            for child in named(node) {
                collect_raises(child, raises);
            }
        }
    }
}

fn raised_value(raise: Node<'_>) -> Option<Node<'_>> {
    let cause = raise.child_by_field_name(CAUSE_FIELD).map(|cause| cause.id());
    named(raise)
        .into_iter()
        .find(|child| child.kind() != COMMENT && Some(child.id()) != cause)
        .map(unparenthesized)
}
