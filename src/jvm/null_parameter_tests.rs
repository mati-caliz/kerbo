use std::path::Path;

use super::library::Library;
use super::null_parameters::requires_non_null;

const CHECKS: &str = "sample.inferred.Checks";
const CONSTRUCTOR: &str = "<init>";

fn library() -> Library {
    Library::load(&[Path::new(env!("CARGO_MANIFEST_DIR")).join("src/jvm/fixtures/inferred.jar")])
}

fn inferred(library: &Library, name: &str, index: usize) -> bool {
    let class = library.class(CHECKS).expect("la clase está");
    let method = class
        .methods
        .iter()
        .find(|method| method.name == name)
        .expect("el método está");
    requires_non_null(library, CHECKS, method, index)
}

fn non_null(library: &Library, parameters: &[(&str, usize)]) -> Vec<String> {
    parameters
        .iter()
        .filter(|(name, index)| inferred(library, name, *index))
        .map(|(name, index)| format!("{name}#{index}"))
        .collect()
}

// Desreferenciar el parámetro en todos los caminos lo hace no nulo: llamarle un método, leerle el
// largo, usarlo de `synchronized`, guardarle un elemento. Los `long` y `double` de antes ocupan dos
// lugares, y las cuentas y los arreglos del medio no lo mueven.
#[test]
fn a_parameter_dereferenced_on_every_path_is_not_null() {
    let library = library();
    let dereferenced = [
        ("direct", 0),
        (CONSTRUCTOR, 0),
        ("sealedLength", 0),
        ("wide", 2),
        ("listed", 0),
        ("synchronizedOn", 0),
        ("stored", 0),
        ("looped", 0),
        ("afterLoop", 0),
        ("switched", 0),
        ("arithmetic", 2),
    ];
    assert_eq!(non_null(&library, &dereferenced).len(), dereferenced.len());
}

// Un camino que termina bien sin tocarlo alcanza para no inferir nada: devolverlo, chequearlo y
// seguir, usarlo en una sola rama, atrapar la excepción, guardarlo en un arreglo o pasarlo a un
// método de interfaz. Un primitivo nunca se infiere, y un método que se puede redefinir tampoco.
#[test]
fn a_parameter_that_survives_null_on_some_path_is_not_inferred() {
    let library = library();
    let tolerated = [
        ("untouched", 0),
        ("tolerant", 0),
        ("oneBranch", 0),
        ("number", 0),
        ("stored", 1),
        ("passedToInterface", 1),
        ("caught", 0),
        ("wide", 0),
        ("overridable", 0),
    ];
    assert_eq!(non_null(&library, &tolerated), Vec::<String>::new());
}

// `if (p == null) throw` y un `instanceof` que no pasa y termina en `throw` cuentan como
// desreferencia; un `finally` que vuelve a tirar no la tapa.
#[test]
fn a_parameter_whose_null_ends_in_a_throw_is_not_null() {
    let library = library();
    assert_eq!(
        non_null(&library, &[("guarded", 0), ("typed", 0), ("cleaned", 0)]),
        ["guarded#0", "typed#0", "cleaned#0"]
    );
}

// Pasarlo a otro método que lo exige no nulo lo hace no nulo, con o sin ramas, si se sabe qué
// método corre: uno estático o uno `final`. Uno que se puede redefinir no dice nada, y uno que se
// llama a sí mismo tampoco.
#[test]
fn a_parameter_passed_to_a_method_that_requires_it_is_not_null() {
    let library = library();
    assert_eq!(
        non_null(
            &library,
            &[
                ("forwarded", 0),
                ("branchedForward", 0),
                ("throughFinal", 1),
                ("throughVirtual", 1),
                ("spin", 0),
            ]
        ),
        ["forwarded#0", "branchedForward#0", "throughFinal#1"]
    );
}

#[test]
fn a_class_without_bytecode_infers_nothing() {
    let library = library();
    let class = library.class(CHECKS).expect("la clase está");
    let method = class.methods.first().expect("tiene métodos");
    assert!(!requires_non_null(&library, "sample.inferred.Missing", method, 0));
    assert!(library.code_at("sample/inferred/Missing").is_none());
}
