use std::collections::HashMap;
use std::ptr;

use super::java::{Declaration, JavaSummary, Position, lint_at};
use super::java_nullness::Resolved;
use super::java_project::IndexedSummary;
use super::java_throw_lookup::ThrowLookup;
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
    declaring: HashMap<&'summary str, Vec<Declaring<'summary>>>,
    caught: HashMap<&'summary str, Vec<Caught<'summary>>>,
}

struct Declaring<'summary> {
    summary: &'summary JavaSummary,
    declaration: &'summary Declaration,
    method: &'summary MethodThrows,
}

struct Caught<'summary> {
    summary: &'summary JavaSummary,
    written: &'summary str,
    origin: &'summary ThrowOrigin,
}

impl<'summary> ThrowsFacts<'summary> {
    pub fn new(files: &[IndexedSummary<'summary>]) -> Self {
        let mut declaring: HashMap<&str, Vec<Declaring<'_>>> = HashMap::new();
        let mut caught: HashMap<&str, Vec<Caught<'_>>> = HashMap::new();
        for file in files {
            let summary = file.summary;
            for (method, declaration) in summary
                .method_throws
                .iter()
                .filter_map(|method| Some((method, declaration_at(summary, method)?)))
            {
                declaring
                    .entry(declaration.name.as_str())
                    .or_default()
                    .push(Declaring {
                        summary,
                        declaration,
                        method,
                    });
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

struct Escaping<'point> {
    exception: Resolved,
    caught: &'point [String],
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
            escaping.extend(thrown.into_iter().map(|exception| Escaping {
                exception,
                caught: point.caught.as_slice(),
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
            && !self.thrown_by_overrider(&declared, declaration)
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

    // Como el análisis global de IntelliJ, una excepción tirada saca de la cláusula tanto a los
    // tipos de los que hereda como a los que heredan de ella.
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
                || self.lookup.is_subclass(declared, exception) != Some(false))
    }

    // Lo que tira el cuerpo de un método que lo redefine, aunque sea de una clase anónima, sube a
    // éste: quien llama por la clase de arriba tiene que poder atraparlo. Un cuerpo que tira algo
    // chequeado lo declara, así que alcanza con mirar los que tienen `throws`.
    fn thrown_by_overrider(&self, declared: &Resolved, declaration: &Declaration) -> bool {
        if declaration.owner.kind == OwnerKind::Local || declaration.is_constructor {
            return false;
        }
        self.facts
            .declaring
            .get(declaration.name.as_str())
            .into_iter()
            .flatten()
            .filter(|overrider| {
                !ptr::eq(overrider.declaration, declaration)
                    && overrider.declaration.parameters.len() == declaration.parameters.len()
                    && self.may_override(overrider, declaration)
            })
            .any(|overrider| self.throws_from_body(overrider, declared))
    }

    fn throws_from_body(&self, overrider: &Declaring<'_>, declared: &Resolved) -> bool {
        let checker = Checker {
            lookup: self.lookup,
            facts: self.facts,
            summary: overrider.summary,
        };
        checker
            .escaping(&overrider.method.points)
            .is_none_or(|escaping| escaping.iter().any(|point| checker.escapes_as(point, declared)))
    }

    // Una clase anónima redefine los métodos del tipo que nombra su `new`; una local con nombre,
    // que no se ve desde afuera, no cuenta.
    fn may_override(&self, overrider: &Declaring<'_>, declaration: &Declaration) -> bool {
        let owner = &overrider.declaration.owner;
        let parent = Resolved::Project(declaration.owner.name.clone());
        if owner.kind == OwnerKind::Local {
            let anonymous_base =
                (!owner.name.is_empty()).then(|| self.lookup.named(overrider.summary, &owner.name));
            return anonymous_base.is_some_and(|base| self.lookup.is_subclass(&base, &parent) != Some(false));
        }
        owner.name != declaration.owner.name
            && self
                .lookup
                .is_subclass(&Resolved::Project(owner.name.clone()), &parent)
                != Some(false)
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
