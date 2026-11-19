use super::class_code::{ClassCode, MethodCode, method_shape};
use super::instructions::decode;
use super::library::Library;
use super::method::Method;
use super::null_partial::{Partial, PassedParameter};
use super::null_paths::{explore, is_branching, straight};

// Hasta dónde se sigue un parámetro de llamada en llamada antes de darlo por no inferido.
const DEPTH_LIMIT: usize = 48;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ParameterKey {
    owner: String,
    name: String,
    descriptor: String,
    index: usize,
}

// Si IntelliJ infiere del bytecode que el parámetro es `@NotNull`: el método lo desreferencia, o
// tira una excepción cuando es null, en todos los caminos que terminan bien (directo o pasándolo a
// otro método que lo exige). Sólo en un método que nadie puede redefinir: de los demás IntelliJ no
// lee el bytecode (`canInferFromByteCode`). `class` es la clase de librería que declara el método.
pub fn requires_non_null(library: &Library, class: &str, method: &Method, index: usize) -> bool {
    library.code_of(class).is_some_and(|code| {
        let is_final = code
            .method(&method.name, &method.descriptor)
            .is_some_and(|declared| code.is_stable(declared));
        if !is_final {
            return false;
        }
        let key = ParameterKey {
            owner: code.name.clone(),
            name: method.name.clone(),
            descriptor: method.descriptor.clone(),
            index,
        };
        solve(library, key, 0)
    })
}

// Una clave que se vuelve a pedir mientras se la resuelve (recursión) no se da por no nula, igual
// que una ecuación que el solver de IntelliJ no llega a cerrar.
fn solve(library: &Library, key: ParameterKey, depth: usize) -> bool {
    if let Some(known) = library.inferred_parameters().borrow().get(&key) {
        return known.unwrap_or(false);
    }
    if depth > DEPTH_LIMIT {
        return false;
    }
    library
        .inferred_parameters()
        .borrow_mut()
        .insert(key.clone(), None);
    let non_null = infer(library, &key, depth);
    library
        .inferred_parameters()
        .borrow_mut()
        .insert(key, Some(non_null));
    non_null
}

fn infer(library: &Library, key: &ParameterKey, depth: usize) -> bool {
    let Some(code) = library.code_at(&key.owner) else {
        return false;
    };
    let Some(partial) = code
        .method(&key.name, &key.descriptor)
        .and_then(|method| parameter_partial(&code, method, key.index))
    else {
        return false;
    };
    match partial {
        Partial::Dereference => true,
        Partial::Identity | Partial::Return => false,
        Partial::Conditional(products) => products.iter().all(|product| {
            product
                .iter()
                .any(|passed| passed_requires_non_null(library, passed, depth))
        }),
    }
}

// Una llamada virtual a un método que se puede redefinir no dice nada: no se sabe cuál corre.
fn passed_requires_non_null(library: &Library, passed: &PassedParameter, depth: usize) -> bool {
    let target = &passed.target;
    let Some(code) = library.code_at(&target.owner) else {
        return false;
    };
    let is_known = code
        .method(&target.name, &target.descriptor)
        .is_some_and(|method| passed.stable_call || code.is_stable(method));
    is_known
        && solve(
            library,
            ParameterKey {
                owner: target.owner.clone(),
                name: target.name.clone(),
                descriptor: target.descriptor.clone(),
                index: passed.index,
            },
            depth + 1,
        )
}

// La local donde el método recibe ese parámetro, si es una referencia.
pub(super) fn parameter_local(method: &MethodCode, index: usize) -> Option<usize> {
    let shape = method_shape(&method.descriptor)?;
    if !shape.arguments.get(index)?.is_reference {
        return None;
    }
    let receiver_slots = usize::from(!method.is_static());
    let preceding: usize = shape
        .arguments
        .iter()
        .take(index)
        .map(|argument| argument.width)
        .sum();
    Some(receiver_slots + preceding)
}

fn parameter_partial(code: &ClassCode, method: &MethodCode, index: usize) -> Option<Partial> {
    let parameter_local = parameter_local(method, index)?;
    let body = decode(&code.pool, method.code.as_ref()?)?;
    if is_branching(&body) {
        explore(&body, parameter_local)
    } else {
        straight(&body, parameter_local)
    }
}
