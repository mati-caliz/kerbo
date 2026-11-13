use super::java_chains::Chain;
use super::java_null_inference::NullArgument;

// Un uso de una variable que quedaría sin proteger si la llamada que le dio su valor devolviera
// null: el método o campo que se usa y dónde.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dereference {
    pub name: String,
    pub line: usize,
    pub column: usize,
}

// Una variable asignada desde una llamada (`Cache cache = manager.getCache(nombre)`), los usos
// que no tienen un chequeo de null por delante en ningún camino y las llamadas a las que se pasa
// sin chequear. Después de pasarla a una llamada, los usos van a otro flujo que recuerda esa
// llamada: si el parámetro resulta no nulo, IntelliJ ya la da por presente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NullFlow {
    pub origin: NullOrigin,
    pub dereferences: Vec<Dereference>,
    pub arguments: Vec<NullArgument>,
    pub passed_before: Vec<NullArgument>,
}

// De dónde puede venir el null: una llamada que puede devolverlo, un campo anotado `@Nullable` o el
// campo de otro objeto, con el tipo de ese objeto como está escrito, el campo arreglo por el que se
// llega a él si lo hay (`mapa.segmentos[0].tabla`) y si se volvió a leer después de una llamada
// (entonces cuenta sólo si el campo es `volatile`, y eso se sabe en el índice).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NullOrigin {
    Call(Chain),
    NullableField,
    QualifiedField {
        owner: String,
        via: Option<String>,
        field: String,
        after_call: bool,
    },
}
