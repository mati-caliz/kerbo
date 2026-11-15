use tree_sitter::Node;

use super::java_chains::{Chain, origin_chain};
use super::java_null_conditions::without_cast;
use super::java_null_flow::{
    ARGUMENTS_FIELD, Analysis, COMMENTS, INVOCATION, NullOrigin, OBJECT_FIELD, State,
};
use super::java_null_keys::tracked_key;
use super::java_null_uses::is_nullable_element;
use crate::syntax::named_children;

// La cadena que termina en la llamada y, si es una llamada sin receptor, de dónde puede venir el
// null de cada argumento en este punto del método: una variable que el flujo sigue, el elemento de
// un arreglo `@Nullable` u otra llamada.
pub(super) fn forwarding_chain(
    analysis: &Analysis<'_, '_>,
    invocation: Node<'_>,
    state: &State,
) -> Option<Chain> {
    let mut chain = origin_chain(invocation, analysis.source, analysis.variables)?;
    chain.forwarded = forwarded_sources(analysis, invocation, state);
    Some(chain)
}

fn forwarded_sources(
    analysis: &Analysis<'_, '_>,
    invocation: Node<'_>,
    state: &State,
) -> Vec<(usize, NullOrigin)> {
    if invocation.child_by_field_name(OBJECT_FIELD).is_some() {
        return Vec::new();
    }
    invocation
        .child_by_field_name(ARGUMENTS_FIELD)
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .filter(|argument| !COMMENTS.contains(&argument.kind()))
        .enumerate()
        .filter_map(|(index, argument)| {
            let argument = without_cast(argument);
            let tracked = tracked_key(analysis, argument).and_then(|key| state.get(&key).copied());
            let origin = match tracked {
                Some(flow) => analysis.flows.get(flow)?.origin.clone(),
                None if is_nullable_element(analysis, argument) => NullOrigin::Annotated,
                None if argument.kind() == INVOCATION => {
                    NullOrigin::Call(forwarding_chain(analysis, argument, state)?)
                }
                None => return None,
            };
            Some((index, origin))
        })
        .collect()
}
