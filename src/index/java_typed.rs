use std::collections::HashMap;

use super::java_nullness::Resolved;
use crate::jvm::class_file::{ClassFile, binary_to_source};
use crate::jvm::generics::{Generic, class_signature, field_signature, method_signature};
use crate::jvm::method::Method;

// Un tipo con sus argumentos (`List<Fallo>`), para seguir una cadena que pasa por un genérico. Un
// argumento que no se sabe queda vacío y deja al método con lo que dice el descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Typed {
    pub class: Resolved,
    pub arguments: Vec<Option<Self>>,
}

impl Typed {
    pub const fn raw(class: Resolved) -> Self {
        Self {
            class,
            arguments: Vec::new(),
        }
    }
}

type Bindings = HashMap<String, Typed>;

const OBJECT_START: char = 'L';
const OBJECT_END: char = ';';

// Los argumentos de `current` puestos en los parámetros de tipo de su clase. Un tipo crudo o con
// otra cantidad de argumentos no liga nada.
fn bindings_of(file: &ClassFile, current: &Typed) -> Bindings {
    let Some(signature) = file.signature.as_deref().and_then(class_signature) else {
        return Bindings::new();
    };
    if signature.parameters.len() != current.arguments.len() {
        return Bindings::new();
    }
    signature
        .parameters
        .into_iter()
        .zip(&current.arguments)
        .filter_map(|(name, argument)| Some((name, argument.clone()?)))
        .collect()
}

// De quién hereda una clase de librería, con los argumentos que le pasa (`ArrayList<E>` a `List<E>`).
pub fn library_parents(file: &ClassFile, current: &Typed) -> Vec<Typed> {
    let Some(signature) = file.signature.as_deref().and_then(class_signature) else {
        let mut parents: Vec<Typed> = file
            .interfaces
            .iter()
            .cloned()
            .map(|name| Typed::raw(Resolved::Library(name)))
            .collect();
        parents.extend(
            file.superclass
                .iter()
                .cloned()
                .map(|name| Typed::raw(Resolved::Library(name))),
        );
        return parents;
    };
    let bindings = bindings_of(file, current);
    signature
        .parents
        .iter()
        .filter_map(|parent| substituted(parent, &bindings, &[]))
        .collect()
}

// Lo que devuelve un método de librería llamado sobre `current`: `List<Fallo>.get` da `Fallo`. Si la
// firma no lo dice, lo del descriptor (`Object` para un `E` sin ligar).
pub fn library_returned(file: &ClassFile, method: &Method, current: &Typed) -> Option<Typed> {
    let erased = method
        .return_class()
        .map(|name| Typed::raw(Resolved::Library(name)));
    let Some(signature) = method.signature.as_deref().and_then(method_signature) else {
        return erased;
    };
    substituted(
        &signature.returned,
        &bindings_of(file, current),
        &signature.parameters,
    )
    .or(erased)
}

// El tipo de un campo de una clase de librería (`StandardCharsets.UTF_8` es un `Charset`).
pub fn library_field(file: &ClassFile, name: &str) -> Option<Typed> {
    let field = file.fields.iter().find(|field| field.name == name)?;
    field
        .signature
        .as_deref()
        .and_then(field_signature)
        .and_then(|generic| substituted(&generic, &Bindings::new(), &[]))
        .or_else(|| {
            let binary = field
                .descriptor
                .strip_prefix(OBJECT_START)?
                .strip_suffix(OBJECT_END)?;
            Some(Typed::raw(Resolved::Library(binary_to_source(binary))))
        })
}

// `hidden` son los parámetros de tipo del propio método, que tapan a los de la clase.
fn substituted(generic: &Generic, bindings: &Bindings, hidden: &[String]) -> Option<Typed> {
    match generic {
        Generic::Class(name, arguments) => Some(Typed {
            class: Resolved::Library(name.clone()),
            arguments: arguments
                .iter()
                .map(|argument| substituted(argument, bindings, hidden))
                .collect(),
        }),
        Generic::Variable(name) if !hidden.contains(name) => bindings.get(name).cloned(),
        Generic::Variable(_) | Generic::Other => None,
    }
}
