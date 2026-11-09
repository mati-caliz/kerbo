use std::collections::HashMap;

use super::java::{Declaration, JavaSummary};
use super::java_nullness::Resolved;
use super::java_throw_lookup::{Signature, ThrowLookup, accepts, declared_clauses};
use super::java_throw_wanted::Wanted;
use super::java_typed::Typed;
use crate::lint::java_calls::ReturnType;
use crate::lint::java_generics::{GenericParent, TypeGenerics};
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
                        && accepts(declaration, wanted.arity)
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
    let substitution = Substitution {
        lookup,
        summary,
        bindings,
        hidden: method_parameters(summary, declaration),
    };
    Signature {
        thrown: declared_clauses(summary, declaration)
            .map(|written| substitution.thrown(written))
            .collect(),
        returned: substitution.returned(declaration),
    }
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

fn method_parameters<'summary>(
    summary: &'summary JavaSummary,
    declaration: &Declaration,
) -> &'summary [String] {
    summary
        .generics
        .methods
        .iter()
        .find(|method| {
            method.line == declaration.position.line && method.column == declaration.position.column
        })
        .map_or(&[], |method| method.parameters.as_slice())
}
