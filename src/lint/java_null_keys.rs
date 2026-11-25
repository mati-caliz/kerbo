use tree_sitter::Node;

use super::java_fields::Volatility;
use super::java_inherited_parameters::inherited_parameters;
use super::java_null_flow::{Analysis, IDENTIFIER, NullFlow, NullOrigin, OBJECT_FIELD, State};
use super::java_nullable_fields::{nullable_element_parameters, nullable_field_keys, nullable_parameters};
use crate::syntax::descendants;

const FIELD_ACCESS: &str = "field_access";
const THIS: &str = "this";
const FIELD_FIELD: &str = "field";
const ARRAY_ACCESS: &str = "array_access";
const ARRAY_FIELD: &str = "array";

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

// El nombre con que el estado sigue a la expresión: una variable, `this.campo` o el campo de una
// variable de tipo conocido (`segmento.tabla`, `mapa.segmentos[0].tabla`).
pub(super) fn tracked_key(analysis: &Analysis<'_, '_>, node: Node<'_>) -> Option<String> {
    match node.kind() {
        IDENTIFIER => Some(analysis.text(node).to_owned()),
        FIELD_ACCESS => {
            let object = node.child_by_field_name(OBJECT_FIELD)?;
            if object.kind() == THIS {
                let field = node.child_by_field_name(FIELD_FIELD)?;
                return Some(format!("{THIS}.{}", analysis.text(field)));
            }
            qualified_access(analysis, node).map(|access| access.key)
        }
        _ => None,
    }
}

// El campo de otra variable que el flujo sigue: su clave, el tipo de la variable y, si se llega por
// un elemento de un arreglo que es campo de la variable (`mapa.segmentos[0].tabla`), ese campo.
struct QualifiedAccess {
    key: String,
    owner: String,
    via: Option<String>,
    field: String,
}

fn qualified_access(analysis: &Analysis<'_, '_>, access: Node<'_>) -> Option<QualifiedAccess> {
    let object = access.child_by_field_name(OBJECT_FIELD)?;
    let field = analysis.text(access.child_by_field_name(FIELD_FIELD)?).to_owned();
    let (variable, via) = match object.kind() {
        IDENTIFIER => (object, None),
        ARRAY_ACCESS => {
            let array = object
                .child_by_field_name(ARRAY_FIELD)
                .filter(|array| array.kind() == FIELD_ACCESS)?;
            let variable = array
                .child_by_field_name(OBJECT_FIELD)
                .filter(|variable| variable.kind() == IDENTIFIER)?;
            let via = analysis.text(array.child_by_field_name(FIELD_FIELD)?).to_owned();
            (variable, Some(via))
        }
        _ => return None,
    };
    Some(QualifiedAccess {
        key: format!("{}.{field}", analysis.text(object)),
        owner: qualified_owner(analysis, variable)?.clone(),
        via,
        field,
    })
}

// Un campo `@Nullable` que puede ser `volatile`: las claves con que se lo lee, de dónde sale el null
// al releerlo y si ya se sabe que es `volatile` (los de otras variables se saben en el índice).
pub(super) struct VolatileField {
    keys: Vec<String>,
    origin: NullOrigin,
    known_volatile: bool,
}

// La clave y, si es un campo, la otra forma de escribirlo.
pub(super) fn spellings(analysis: &Analysis<'_, '_>, key: String) -> Vec<String> {
    let partner = analysis.partners.get(&key).cloned();
    std::iter::once(key).chain(partner).collect()
}

// El tipo con que se declara la variable que califica un campo.
fn qualified_owner<'context>(
    analysis: &Analysis<'_, 'context>,
    object: Node<'_>,
) -> Option<&'context String> {
    analysis.variables.get(analysis.text(object))?.as_ref()
}

// Al entrar al método, cada campo y cada parámetro `@Nullable` que ve puede ser null. Los campos de
// otras variables (`segmento.tabla`) también: si su clase los declara `@Nullable` se sabe recién en
// el índice. Los elementos de un parámetro arreglo `@Nullable` no se siguen: cada lectura puede ser
// null.
pub(super) fn seed_nullable_variables(analysis: &mut Analysis<'_, '_>, method: Node<'_>) -> State {
    let mut state = State::new();
    seed_qualified_fields(analysis, method, &mut state);
    analysis.nullable_elements = nullable_element_parameters(method, analysis.source);
    for parameter in nullable_parameters(method, analysis.source) {
        state.insert(parameter, analysis.flows.len());
        analysis.flows.push(NullFlow::new(NullOrigin::Annotated));
    }
    for inherited in inherited_parameters(method, analysis.source) {
        if state.contains_key(&inherited.name) {
            continue;
        }
        state.insert(inherited.name, analysis.flows.len());
        analysis.flows.push(NullFlow::new(inherited.origin));
    }
    for (keys, volatility) in nullable_field_keys(method, analysis.source) {
        if volatility == Volatility::Volatile {
            analysis.volatile_fields.push(VolatileField {
                keys: keys.clone(),
                origin: NullOrigin::Annotated,
                known_volatile: true,
            });
        }
        if let [qualified, bare] = keys.as_slice() {
            analysis.partners.insert(qualified.clone(), bare.clone());
            analysis.partners.insert(bare.clone(), qualified.clone());
        }
        for key in keys {
            state.insert(key, analysis.flows.len());
        }
        analysis.flows.push(NullFlow::new(NullOrigin::Annotated));
    }
    state
}

fn seed_qualified_fields(analysis: &mut Analysis<'_, '_>, method: Node<'_>, state: &mut State) {
    for access in descendants(method)
        .into_iter()
        .filter(|node| node.kind() == FIELD_ACCESS)
    {
        let Some(QualifiedAccess {
            key,
            owner,
            via,
            field,
        }) = qualified_access(analysis, access)
        else {
            continue;
        };
        if state.contains_key(&key) {
            continue;
        }
        analysis.volatile_fields.push(VolatileField {
            keys: vec![key.clone()],
            origin: NullOrigin::QualifiedField {
                owner: owner.clone(),
                via: via.clone(),
                field: field.clone(),
                after_call: true,
            },
            known_volatile: false,
        });
        state.insert(key, analysis.flows.len());
        analysis.flows.push(NullFlow::new(NullOrigin::QualifiedField {
            owner,
            via,
            field,
            after_call: false,
        }));
    }
}

// IntelliJ no sigue los campos `volatile`: cada lectura es un valor nuevo que puede ser null aunque
// se haya chequeado, desreferenciado o asignado. Un campo propio `volatile` se relee siempre; el de
// otra variable (`segmento.tabla`), si el flujo ya no lo sigue, en un flujo que cuenta sólo si su
// clase lo declara `volatile` (eso se sabe en el índice).
pub(super) fn reread(analysis: &mut Analysis<'_, '_>, mut state: State, key: &str) -> State {
    let Some(field) = analysis
        .volatile_fields
        .iter()
        .find(|field| field.keys.iter().any(|known| known == key))
    else {
        return state;
    };
    let still_followed = field.keys.iter().any(|known| state.contains_key(known));
    if !field.known_volatile && still_followed {
        return state;
    }
    let (keys, origin) = (field.keys.clone(), field.origin.clone());
    for spelling in keys {
        state.insert(spelling, analysis.flows.len());
    }
    analysis.flows.push(NullFlow::new(origin));
    state
}
