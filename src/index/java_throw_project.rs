use std::collections::HashMap;
use std::ptr;

use super::java::{Declaration, JavaSummary};
use super::java_nullness::Resolved;
use super::java_throw_lookup::{Signature, ThrowLookup, declared_clauses};
use super::java_throw_wanted::Wanted;
use super::java_typed::Typed;
use crate::lint::java_calls::{Body, ReturnType};
use crate::lint::java_generics::{Carrier, GenericParent, MethodGenerics, TypeGenerics};
use crate::lint::java_receivers::UNKNOWN_TYPE_ARGUMENT;

const JAVA_OBJECT: &str = "java.lang.Object";

type Bindings = HashMap<String, Typed>;

// Los tipos del proyecto con sus genéricos: `ConcurrentInitializer<T> extends FailableSupplier<T,
// ConcurrentException>` hace que el `get() throws E` heredado tire `ConcurrentException`. Un tipo
// crudo, con otra cantidad de argumentos o declarado dos veces en el archivo no liga nada (y el
// crudo hereda crudo), y el `E` sin ligar sigue sin saberse qué es.
pub(super) fn visit_project(
    lookup: &ThrowLookup<'_, '_>,
    owner: &str,
    current: &Typed,
    wanted: Wanted<'_>,
) -> (Vec<Signature>, Vec<Typed>) {
    let mut found = Vec::new();
    let mut parents = vec![Typed::raw(Resolved::Library(JAVA_OBJECT.to_owned()))];
    for (summary, written_parents) in lookup.types.declared_in(owner) {
        let generics = generics_in(summary, owner);
        let bindings = bindings_of(generics, &current.arguments);
        let raw = generics.is_some_and(|declared| declared.parameters.len() != current.arguments.len());
        found.extend(
            summary
                .declarations
                .iter()
                .filter(|declaration| {
                    declaration.owner.name == owner
                        && !declaration.is_constructor
                        && declaration.name == wanted.name
                        && wanted.admits(declaration)
                })
                .map(|declaration| project_signature(lookup, summary, declaration, &bindings)),
        );
        match generics.filter(|_| !raw) {
            Some(declared) => {
                let substitution = Substitution {
                    lookup,
                    summary,
                    bindings: &bindings,
                    hidden: &[],
                };
                parents.extend(declared.parents.iter().map(|parent| substitution.parent(parent)));
            }
            None => parents.extend(
                written_parents
                    .iter()
                    .map(|parent| Typed::raw(lookup.parent_named(summary, parent))),
            ),
        }
    }
    (found, parents)
}

pub(super) fn raw_signature(
    lookup: &ThrowLookup<'_, '_>,
    summary: &JavaSummary,
    declaration: &Declaration,
) -> Signature {
    project_signature(lookup, summary, declaration, &Bindings::new())
}

fn project_signature(
    lookup: &ThrowLookup<'_, '_>,
    summary: &JavaSummary,
    declaration: &Declaration,
    bindings: &Bindings,
) -> Signature {
    let generics = method_generics(summary, declaration);
    let substitution = Substitution {
        lookup,
        summary,
        bindings,
        hidden: generics.map_or(&[], |method| method.parameters.as_slice()),
    };
    let mut thrown = Vec::new();
    let mut carried = Vec::new();
    let mut unresolved = false;
    for written in declared_clauses(summary, declaration) {
        match generics.and_then(|method| carried_positions(lookup, summary, method, written)) {
            Some(positions) => carried.push(positions),
            None if substitution.is_unbound_parameter(written) => unresolved = true,
            None => thrown.push(substitution.thrown(written)),
        }
    }
    Signature {
        thrown,
        returned: substitution.returned(declaration),
        carried,
        unresolved,
    }
}

// Las posiciones de los argumentos que dicen qué es el `E` de un `throws E` del método, si todos
// los que lo llevan son interfaces funcionales del proyecto cuyo único método abstracto lo tira.
fn carried_positions(
    lookup: &ThrowLookup<'_, '_>,
    summary: &JavaSummary,
    method: &MethodGenerics,
    written: &str,
) -> Option<Vec<usize>> {
    let carriers: Vec<&Carrier> = method
        .carriers
        .iter()
        .filter(|carrier| carrier.parameter == written)
        .collect();
    (!carriers.is_empty()
        && carriers
            .iter()
            .all(|carrier| throws_argument(lookup, summary, carrier)))
    .then(|| carriers.iter().map(|carrier| carrier.position).collect())
}

fn throws_argument(lookup: &ThrowLookup<'_, '_>, summary: &JavaSummary, carrier: &Carrier) -> bool {
    let Resolved::Project(interface) = lookup.named(summary, &carrier.interface) else {
        return false;
    };
    let Some(file) = lookup.types.declaring_file(&interface, summary) else {
        return false;
    };
    let Some(thrown_parameter) =
        generics_in(file, &interface).and_then(|declared| declared.parameters.get(carrier.argument))
    else {
        return false;
    };
    let abstract_methods: Vec<(&JavaSummary, &Declaration)> = lookup
        .types
        .declarations_of(&interface)
        .filter(|(owner_file, declaration)| {
            ptr::eq(*owner_file, file) && !declaration.is_constructor && declaration.body == Body::Abstract
        })
        .collect();
    matches!(
        abstract_methods.as_slice(),
        [(owner_file, abstract_method)]
            if !method_parameters(owner_file, abstract_method).contains(thrown_parameter)
                && declared_clauses(owner_file, abstract_method).any(|clause| clause == thrown_parameter)
    )
}

// Lo que vale un nombre escrito en un archivo con los parámetros de tipo ya ligados; los del método
// tapan a los de la clase.
struct Substitution<'context, 'query, 'summary> {
    lookup: &'context ThrowLookup<'query, 'summary>,
    summary: &'context JavaSummary,
    bindings: &'context Bindings,
    hidden: &'context [String],
}

impl Substitution<'_, '_, '_> {
    fn bound(&self, written: &str) -> Option<&Typed> {
        (!self.hidden.iter().any(|parameter| parameter == written))
            .then(|| self.bindings.get(written))
            .flatten()
    }

    // Un `throws X` con `X` parámetro de tipo sin ligar lo decide quien llama (`Class<X>`, otro
    // genérico): no es una clase que se llame `X`.
    fn is_unbound_parameter(&self, written: &str) -> bool {
        self.bound(written).is_none() && is_type_parameter(self.summary, written)
    }

    fn thrown(&self, written: &str) -> Resolved {
        self.bound(written).map_or_else(
            || self.lookup.named(self.summary, written),
            |bound| bound.class.clone(),
        )
    }

    fn returned(&self, declaration: &Declaration) -> Option<Typed> {
        if declaration.return_type != ReturnType::Reference {
            return None;
        }
        self.bound(&declaration.returned)
            .cloned()
            .or_else(|| self.written(&declaration.returned, &declaration.returned_arguments))
    }

    fn parent(&self, parent: &GenericParent) -> Typed {
        Typed {
            class: self.lookup.parent_named(self.summary, &parent.written),
            arguments: self.arguments(&parent.arguments),
        }
    }

    fn written(&self, written: &str, arguments: &[String]) -> Option<Typed> {
        (!is_type_parameter(self.summary, written)).then(|| Typed {
            class: self.lookup.named(self.summary, written),
            arguments: self.arguments(arguments),
        })
    }

    fn arguments(&self, arguments: &[String]) -> Vec<Option<Typed>> {
        arguments.iter().map(|argument| self.argument(argument)).collect()
    }

    fn argument(&self, written: &str) -> Option<Typed> {
        self.bound(written).cloned().or_else(|| {
            (written != UNKNOWN_TYPE_ARGUMENT && !is_type_parameter(self.summary, written))
                .then(|| Typed::raw(self.lookup.named(self.summary, written)))
        })
    }
}

// Un nombre que en el archivo es parámetro de tipo de alguna clase o método no es una clase: sin
// saber cuál de todos es, no se resuelve.
pub(super) fn is_type_parameter(summary: &JavaSummary, written: &str) -> bool {
    let generics = &summary.generics;
    generics
        .types
        .iter()
        .flat_map(|declared| &declared.parameters)
        .chain(generics.methods.iter().flat_map(|method| &method.parameters))
        .any(|parameter| parameter == written)
}

fn generics_in<'summary>(summary: &'summary JavaSummary, owner: &str) -> Option<&'summary TypeGenerics> {
    let declared_once = summary.hierarchy.iter().filter(|(name, _)| name == owner).count() == 1;
    let mut matching = summary
        .generics
        .types
        .iter()
        .filter(|declared| declared.name == owner);
    let found = matching.next();
    (declared_once && matching.next().is_none())
        .then_some(found)
        .flatten()
}

fn bindings_of(generics: Option<&TypeGenerics>, arguments: &[Option<Typed>]) -> Bindings {
    generics
        .filter(|declared| declared.parameters.len() == arguments.len())
        .map(|declared| {
            declared
                .parameters
                .iter()
                .zip(arguments)
                .filter_map(|(parameter, argument)| Some((parameter.clone(), argument.clone()?)))
                .collect()
        })
        .unwrap_or_default()
}

fn method_generics<'summary>(
    summary: &'summary JavaSummary,
    declaration: &Declaration,
) -> Option<&'summary MethodGenerics> {
    summary.generics.methods.iter().find(|method| {
        method.line == declaration.position.line && method.column == declaration.position.column
    })
}

fn method_parameters<'summary>(
    summary: &'summary JavaSummary,
    declaration: &Declaration,
) -> &'summary [String] {
    method_generics(summary, declaration).map_or(&[], |method| method.parameters.as_slice())
}
