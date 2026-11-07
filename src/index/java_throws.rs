use std::collections::HashMap;
use std::ptr;

use super::java::{Declaration, JavaSummary, Position, lint_at};
use super::java_nullness::Resolved;
use super::java_project::IndexedSummary;
use super::java_throw_lookup::{ThrowLookup, declared_clauses};
use crate::lint::LintFinding;
use crate::lint::java_calls::Body;
use crate::lint::java_owner::OwnerKind;
use crate::lint::java_throws::{MethodThrows, ThrowOrigin, ThrowPoint, ThrowsClause};
use crate::rules::JavaRule;

const RUNTIME_EXCEPTION: &str = "java.lang.RuntimeException";
const ERROR: &str = "java.lang.Error";
const REMOTE_EXCEPTION: &str = "java.rmi.RemoteException";
const GENERAL_EXCEPTIONS: [&str; 3] = ["java.lang.Throwable", "java.lang.Exception", "java.lang.Error"];
const SERIALIZATION_METHODS: [&str; 5] = [
    "writeObject",
    "readObject",
    "readObjectNoData",
    "writeReplace",
    "readResolve",
];

// Lo que hace falta saber del resto del proyecto: quién más declara un `throws` con ese nombre de
// método, y qué llamadas con ese nombre están adentro de un `try`.
pub struct ThrowsFacts<'summary> {
    declaring: HashMap<&'summary str, Vec<(&'summary JavaSummary, &'summary Declaration)>>,
    caught: HashMap<&'summary str, Vec<Caught<'summary>>>,
}

struct Caught<'summary> {
    summary: &'summary JavaSummary,
    written: &'summary str,
    origin: &'summary ThrowOrigin,
}

impl<'summary> ThrowsFacts<'summary> {
    pub fn new(files: &[IndexedSummary<'summary>]) -> Self {
        let mut declaring: HashMap<&str, Vec<(&JavaSummary, &Declaration)>> = HashMap::new();
        let mut caught: HashMap<&str, Vec<Caught<'_>>> = HashMap::new();
        for file in files {
            let summary = file.summary;
            for declaration in summary
                .method_throws
                .iter()
                .filter_map(|method| declaration_at(summary, method))
            {
                declaring
                    .entry(declaration.name.as_str())
                    .or_default()
                    .push((summary, declaration));
            }
            for call in &summary.caught_calls {
                caught
                    .entry(call.name.as_str())
                    .or_default()
                    .extend(call.caught.iter().map(|written| Caught {
                        summary,
                        written: written.as_str(),
                        origin: &call.origin,
                    }));
            }
        }
        Self { declaring, caught }
    }
}

fn declaration_at<'summary>(
    summary: &'summary JavaSummary,
    method: &MethodThrows,
) -> Option<&'summary Declaration> {
    summary.declarations.iter().find(|declaration| {
        declaration.position.line == method.line && declaration.position.column == method.column
    })
}

// Calca RedundantThrows de IntelliJ, que no marca si le queda una llamada sin resolver: kerbo tampoco
// opina de un método con algo que no sabe resolver.
pub fn redundant_throws(
    lookup: &ThrowLookup<'_, '_>,
    facts: &ThrowsFacts<'_>,
    summary: &JavaSummary,
) -> Vec<LintFinding> {
    let checker = Checker {
        lookup,
        facts,
        summary,
    };
    summary
        .method_throws
        .iter()
        .flat_map(|method| checker.method_findings(method))
        .collect()
}

struct Checker<'check, 'query, 'summary> {
    lookup: &'check ThrowLookup<'query, 'summary>,
    facts: &'check ThrowsFacts<'summary>,
    summary: &'check JavaSummary,
}

// Lo que sale de un punto del cuerpo. `widened` es una variable relanzada: puede llevar cualquier
// subtipo de su tipo declarado.
struct Escaping<'point> {
    exception: Resolved,
    caught: &'point [String],
    widened: bool,
}

impl Checker<'_, '_, '_> {
    fn method_findings(&self, method: &MethodThrows) -> Vec<LintFinding> {
        let Some(declaration) = declaration_at(self.summary, method).filter(|found| {
            found.body == Body::Written && !SERIALIZATION_METHODS.contains(&found.name.as_str())
        }) else {
            return Vec::new();
        };
        let Some(escaping) = self.escaping(&method.points) else {
            return Vec::new();
        };
        method
            .clauses
            .iter()
            .filter(|clause| self.is_redundant(clause, declaration, &escaping))
            .map(|clause| {
                lint_at(
                    JavaRule::RedundantThrows,
                    &Position {
                        line: clause.line,
                        column: clause.column,
                    },
                    clause.written.clone(),
                )
            })
            .collect()
    }

    fn escaping<'point>(&self, points: &'point [ThrowPoint]) -> Option<Vec<Escaping<'point>>> {
        let mut escaping = Vec::new();
        for point in points {
            let thrown = self.lookup.thrown_by(self.summary, &point.origin)?;
            let widened = matches!(point.origin, ThrowOrigin::Rethrown(_));
            escaping.extend(thrown.into_iter().map(|exception| Escaping {
                exception,
                caught: point.caught.as_slice(),
                widened,
            }));
        }
        Some(escaping)
    }

    fn is_redundant(
        &self,
        clause: &ThrowsClause,
        declaration: &Declaration,
        escaping: &[Escaping<'_>],
    ) -> bool {
        let declared = self.lookup.named(self.summary, &clause.written);
        self.is_checked(&declared)
            && declared != Resolved::Library(REMOTE_EXCEPTION.to_owned())
            && !escaping.iter().any(|point| self.escapes_as(point, &declared))
            && !self.declared_by_overrider(&declared, declaration)
            && !self.caught_by_caller(&declared, declaration)
    }

    fn is_checked(&self, declared: &Resolved) -> bool {
        [RUNTIME_EXCEPTION, ERROR].iter().all(|unchecked| {
            self.lookup
                .is_subclass(declared, &Resolved::Library((*unchecked).to_owned()))
                == Some(false)
        })
    }

    // Sólo cuenta lo chequeado: un `throw new RuntimeException()` no justifica un `throws Exception`.
    fn is_known_unchecked(&self, exception: &Resolved) -> bool {
        [RUNTIME_EXCEPTION, ERROR].iter().any(|unchecked| {
            self.lookup
                .is_subclass(exception, &Resolved::Library((*unchecked).to_owned()))
                == Some(true)
        })
    }

    fn escapes_as(&self, point: &Escaping<'_>, declared: &Resolved) -> bool {
        let exception = &point.exception;
        if self.is_known_unchecked(exception) {
            return false;
        }
        let handled = point.caught.iter().any(|written| {
            let handler = self.lookup.named(self.summary, written);
            self.lookup.is_subclass(exception, &handler) == Some(true)
        });
        !handled
            && (self.lookup.is_subclass(exception, declared) != Some(false)
                || point.widened && self.lookup.is_subclass(declared, exception) != Some(false))
    }

    // Un método que lo redefine y declara ese tipo o uno más chico justifica el `throws`: quien
    // llama por la clase de arriba tiene que poder atraparlo.
    fn declared_by_overrider(&self, declared: &Resolved, declaration: &Declaration) -> bool {
        if declaration.owner.kind == OwnerKind::Local || declaration.is_constructor {
            return false;
        }
        self.facts
            .declaring
            .get(declaration.name.as_str())
            .into_iter()
            .flatten()
            .filter(|(_, other)| {
                !ptr::eq(*other, declaration)
                    && other.parameters.len() == declaration.parameters.len()
                    && self.may_override(other, declaration)
            })
            .any(|(summary, other)| {
                declared_clauses(summary, other).any(|written| {
                    let thrown = self.lookup.named(summary, written);
                    self.lookup.is_subclass(&thrown, declared) != Some(false)
                })
            })
    }

    fn may_override(&self, other: &Declaration, declaration: &Declaration) -> bool {
        if other.owner.kind == OwnerKind::Local && other.owner.name == declaration.owner.name {
            return true;
        }
        self.lookup.is_subclass(
            &Resolved::Project(other.owner.name.clone()),
            &Resolved::Project(declaration.owner.name.clone()),
        ) != Some(false)
            && other.owner.name != declaration.owner.name
    }

    fn caught_by_caller(&self, declared: &Resolved, declaration: &Declaration) -> bool {
        self.facts
            .caught
            .get(declaration.name.as_str())
            .into_iter()
            .flatten()
            .filter(|call| self.may_call(call, declaration))
            .any(|call| {
                let handler = self.lookup.named(call.summary, call.written);
                !is_general(&handler) && self.lookup.is_subclass(declared, &handler) != Some(false)
            })
    }

    // Como IntelliJ, el `catch` cuenta para el método al que va la llamada y para los que ese
    // redefine, no para los que lo redefinen. Un método de una clase anónima o local sólo se llama
    // desde su archivo; si el receptor no se sabe, cuenta.
    fn may_call(&self, call: &Caught<'_>, declaration: &Declaration) -> bool {
        if declaration.owner.kind == OwnerKind::Local {
            return ptr::eq(call.summary, self.summary);
        }
        let owner = Resolved::Project(declaration.owner.name.clone());
        self.lookup
            .receivers_of(call.summary, call.origin)
            .is_none_or(|receivers| {
                receivers
                    .iter()
                    .any(|receiver| self.lookup.is_subclass(receiver, &owner) != Some(false))
            })
    }
}

fn is_general(handler: &Resolved) -> bool {
    matches!(handler, Resolved::Library(name) if GENERAL_EXCEPTIONS.contains(&name.as_str()))
}
