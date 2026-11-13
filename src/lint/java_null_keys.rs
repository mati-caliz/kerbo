use tree_sitter::Node;

use super::java_null_flow::{Analysis, IDENTIFIER, NullFlow, NullOrigin, OBJECT_FIELD, State};
use super::java_nullable_fields::nullable_field_keys;

const FIELD_ACCESS: &str = "field_access";
const THIS: &str = "this";
const FIELD_FIELD: &str = "field";

// Saca la variable del estado junto con las que tienen su mismo valor: una copia `y = x` o el
// campo escrito como `x` y como `this.x`.
pub(super) fn forget(state: &mut State, key: &str) -> Option<usize> {
    let origin = state.remove(key)?;
    state.retain(|_, kept| *kept != origin);
    Some(origin)
}

// Si las dos ramas siguen la variable, queda el flujo más viejo: uno nacido en una rama al pasarla a
// un método sólo suma chequeos, así que el viejo es el que llega por algún camino.
pub(super) fn join(first: Option<State>, second: Option<State>) -> Option<State> {
    match (first, second) {
        (Some(mut merged), Some(other)) => {
            for (name, origin) in other {
                merged
                    .entry(name)
                    .and_modify(|kept| *kept = (*kept).min(origin))
                    .or_insert(origin);
            }
            Some(merged)
        }
        (one, None) | (None, one) => one,
    }
}

// El nombre con que el estado sigue a la expresión: una variable o `this.campo`.
pub(super) fn tracked_key(analysis: &Analysis<'_, '_>, node: Node<'_>) -> Option<String> {
    match node.kind() {
        IDENTIFIER => Some(analysis.text(node).to_owned()),
        FIELD_ACCESS => {
            let object = node.child_by_field_name(OBJECT_FIELD)?;
            let field = node.child_by_field_name(FIELD_FIELD)?;
            (object.kind() == THIS).then(|| format!("{THIS}.{}", analysis.text(field)))
        }
        _ => None,
    }
}

// La clave y, si es un campo, la otra forma de escribirlo.
pub(super) fn spellings(analysis: &Analysis<'_, '_>, key: String) -> Vec<String> {
    let partner = analysis.partners.get(&key).cloned();
    std::iter::once(key).chain(partner).collect()
}

// Al entrar al método, cada campo `@Nullable` que ve puede ser null.
pub(super) fn seed_nullable_fields(analysis: &mut Analysis<'_, '_>, method: Node<'_>) -> State {
    let mut state = State::new();
    for keys in nullable_field_keys(method, analysis.source) {
        if let [qualified, bare] = keys.as_slice() {
            analysis.partners.insert(qualified.clone(), bare.clone());
            analysis.partners.insert(bare.clone(), qualified.clone());
        }
        for key in keys {
            state.insert(key, analysis.flows.len());
        }
        analysis.flows.push(NullFlow {
            origin: NullOrigin::NullableField,
            dereferences: Vec::new(),
            arguments: Vec::new(),
            passed_before: Vec::new(),
        });
    }
    state
}
