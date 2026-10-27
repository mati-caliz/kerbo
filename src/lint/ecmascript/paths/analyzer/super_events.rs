use tree_sitter::Node;

use super::super::roles::Place;
use super::Analyzer;
use crate::lint::ecmascript::nodes::{CALL, COMMENT};

const FOR: &str = "for_statement";
const INCREMENT_FIELD: &str = "increment";
const FUNCTION_FIELD: &str = "function";
const RETURN: &str = "return_statement";
const THIS: &str = "this";
const SUPER: &str = "super";

// Los eventos que escuchan `constructor-super` y `no-this-before-super` en el camino actual.

// `onCodePathSegmentLoop` para `constructor-super` y `no-this-before-super`.
pub(super) fn notify_loops(analyzer: &mut Analyzer<'_, '_>) {
    let loops = analyzer.graph.take_loops();
    let Some(derived) = analyzer
        .records
        .last_mut()
        .and_then(|record| record.derived.as_mut())
    else {
        return;
    };
    for (from, to) in loops {
        derived.segment_loop(from, to, &analyzer.graph);
    }
}

// `ThisExpression`, `Super` que no es lo llamado y `ReturnStatement` con valor, al entrar.
pub(super) fn observe_super_enter<'tree>(
    analyzer: &mut Analyzer<'tree, '_>,
    node: Node<'tree>,
    place: Place<'tree>,
) {
    let Some(record) = analyzer.records.last_mut() else {
        return;
    };
    let Some(derived) = record.derived.as_mut() else {
        return;
    };
    let is_callee =
        place.parent.is_some_and(|parent| parent.kind() == CALL) && place.field == Some(FUNCTION_FIELD);
    match node.kind() {
        THIS => derived.this_or_super(node, &record.current, &analyzer.graph),
        SUPER if !is_callee => derived.this_or_super(node, &record.current, &analyzer.graph),
        RETURN
            if node
                .named_children(&mut node.walk())
                .any(|child| child.kind() != COMMENT) =>
        {
            derived.return_with_value(&record.current);
        }
        _ => {}
    }
}

// `super(...)` al salir de la llamada.
pub(super) fn observe_super_call<'tree>(analyzer: &mut Analyzer<'tree, '_>, node: Node<'tree>) {
    let is_super_call = node.kind() == CALL
        && node
            .child_by_field_name(FUNCTION_FIELD)
            .is_some_and(|function| function.kind() == SUPER);
    if !is_super_call {
        return;
    }
    if let Some(record) = analyzer.records.last_mut()
        && let Some(derived) = record.derived.as_mut()
    {
        derived.super_call(node, &record.current, &analyzer.graph);
    }
}

pub(super) fn is_for_update(place: Place<'_>) -> bool {
    place.parent.is_some_and(|parent| parent.kind() == FOR) && place.field == Some(INCREMENT_FIELD)
}
