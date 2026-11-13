use tree_sitter::Node;

use super::java::text;
use super::java_calls::annotated_with;
use super::java_null_flow::{Analysis, NULL, NullFlow, NullOrigin, PARENTHESIZED, State};
use super::java_null_keys::tracked_key;
use super::java_null_origin::NullReturn;
use super::java_nullable_fields::declares_nullable_type;
use super::java_owner::{NullMarking, null_marking_of};
use super::java_receivers::simple_type_name;
use super::java_type_variables::type_parameter_admits_null;
use crate::syntax::{named_children, one_based};

const RETURN: &str = "return_statement";
const LAMBDA: &str = "lambda_expression";
const METHOD: &str = "method_declaration";
const TYPE_FIELD: &str = "type";
const VOID_TYPE: &str = "void_type";
const BOXED_VOID: &str = "Void";
const PRIMITIVE_TYPES: &[&str] = &["integral_type", "floating_point_type", "boolean_type"];
const PARAMETRIC_NULLNESS: &str = "ParametricNullness";
const NON_NULL_ANNOTATIONS: &[&str] = &["NonNull", "NotNull", "Nonnull"];
const COMMENTS: &[&str] = &["line_comment", "block_comment"];

// La marca de nulos del método, si lo que devuelve sería no nulo bajo `@NullMarked`: ni primitivo
// ni `void`/`Void`, sin `@Nullable` ni `@ParametricNullness`, y si es una variable de tipo, sin
// límite `@Nullable`. Anotado `@NonNull`, es no nulo con cualquier marca.
pub(super) fn method_marking(method: Node<'_>, source: &[u8]) -> Option<NullMarking> {
    if method.kind() != METHOD {
        return None;
    }
    let written = method.child_by_field_name(TYPE_FIELD)?;
    let excluded = written.kind() == VOID_TYPE
        || PRIMITIVE_TYPES.contains(&written.kind())
        || simple_type_name(text(written, source)) == BOXED_VOID
        || declares_nullable_type(method, source)
        || annotated_with(method, PARAMETRIC_NULLNESS, source)
        || type_parameter_admits_null(written, text(written, source), source) == Some(true);
    if excluded {
        return None;
    }
    let declared_non_null = NON_NULL_ANNOTATIONS
        .iter()
        .any(|annotation| annotated_with(method, annotation, source));
    Some(if declared_non_null {
        NullMarking::Marked
    } else {
        null_marking_of(method, source)
    })
}

// Lo que devuelve el `return` donde está la expresión, si el método que la devuelve lo declara no
// nulo. Dentro de una lambda, no.
pub(super) fn returned_by_method(expression: Node<'_>, source: &[u8]) -> Option<NullReturn> {
    let mut statement = expression.parent()?;
    while statement.kind() == PARENTHESIZED {
        statement = statement.parent()?;
    }
    if statement.kind() != RETURN {
        return None;
    }
    let method = std::iter::successors(statement.parent(), Node::parent)
        .find(|node| node.kind() == METHOD || node.kind() == LAMBDA)
        .filter(|node| node.kind() == METHOD)?;
    let marking = method_marking(method, source)?;
    Some(returned(expression, source, marking))
}

fn returned(expression: Node<'_>, source: &[u8], marking: NullMarking) -> NullReturn {
    let start = expression.start_position();
    NullReturn {
        name: text(expression, source).to_owned(),
        line: one_based(start.row),
        column: one_based(start.column),
        marking,
    }
}

// Anota lo que sale por el `return`: un `null` escrito es un flujo propio, y una variable que el
// flujo sigue suma la devolución a su flujo.
pub(super) fn note_return(analysis: &mut Analysis<'_, '_>, statement: Node<'_>, state: &State) {
    let Some(marking) = analysis.return_marking.filter(|_| statement.kind() == RETURN) else {
        return;
    };
    let Some(value) = named_children(statement)
        .into_iter()
        .find(|child| !COMMENTS.contains(&child.kind()))
        .map(unparenthesized)
    else {
        return;
    };
    let noted = returned(value, analysis.source, marking);
    if value.kind() == NULL {
        let mut flow = NullFlow::new(NullOrigin::Literal);
        flow.returns.push(noted);
        analysis.flows.push(flow);
        return;
    }
    let origin = tracked_key(analysis, value).and_then(|key| state.get(&key).copied());
    if let Some(flow) = origin.and_then(|origin| analysis.flows.get_mut(origin)) {
        flow.returns.push(noted);
    }
}

fn unparenthesized(node: Node<'_>) -> Node<'_> {
    let mut current = node;
    while current.kind() == PARENTHESIZED {
        match named_children(current).into_iter().next() {
            Some(inner) => current = inner,
            None => break,
        }
    }
    current
}
