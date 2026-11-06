use std::collections::HashMap;
use std::ptr;

use super::java::{Declaration, JavaSummary, Position, lint_at};
use super::java_nullness::Resolved;
use super::java_project::IndexedSummary;
use super::java_throw_lookup::{ThrowLookup, declared_clauses};
use crate::lint::LintFinding;
use crate::lint::java_calls::Body;
use crate::lint::java_owner::OwnerKind;
use crate::lint::java_throws::{MethodThrows, ThrowPoint, ThrowsClause};
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
    caught: HashMap<&'summary str, Vec<(&'summary JavaSummary, &'summary str)>>,
}

impl<'summary> ThrowsFacts<'summary> {
    pub fn new(files: &[IndexedSummary<'summary>]) -> Self {
        let mut declaring: HashMap<&str, Vec<(&JavaSummary, &Declaration)>> = HashMap::new();
        let mut caught: HashMap<&str, Vec<(&JavaSummary, &str)>> = HashMap::new();
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
                    .extend(call.caught.iter().map(|written| (summary, written.as_str())));
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
// opina de un método con algo que no sabe resolver. Faltan los constructores, porque pedirían seguir
// el `super()` implícito y los inicializadores de los campos.
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

type Escaping<'point> = (Resolved, &'point [String]);

impl Checker<'_, '_, '_> {
    fn method_findings(&self, method: &MethodThrows) -> Vec<LintFinding> {
        let Some(declaration) = declaration_at(self.summary, method).filter(|found| {
            !found.is_constructor
                && found.body == Body::Written
                && !SERIALIZATION_METHODS.contains(&found.name.as_str())
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
            escaping.extend(
                thrown
                    .into_iter()
                    .map(|exception| (exception, point.caught.as_slice())),
            );
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
            && !escaping
                .iter()
                .any(|(exception, caught)| self.escapes_as(exception, caught, &declared))
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

    fn escapes_as(&self, exception: &Resolved, caught: &[String], declared: &Resolved) -> bool {
        let handled = caught.iter().any(|written| {
            let handler = self.lookup.named(self.summary, written);
            self.lookup.is_subclass(exception, &handler) == Some(true)
        });
        !handled && self.lookup.is_subclass(exception, declared) != Some(false)
    }

    // Un método que lo redefine y declara ese tipo o uno más chico justifica el `throws`: quien
    // llama por la clase de arriba tiene que poder atraparlo.
    fn declared_by_overrider(&self, declared: &Resolved, declaration: &Declaration) -> bool {
        if declaration.owner.kind == OwnerKind::Local {
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
            .any(|(summary, written)| {
                let handler = self.lookup.named(summary, written);
                !is_general(&handler) && self.lookup.is_subclass(declared, &handler) != Some(false)
            })
    }
}

fn is_general(handler: &Resolved) -> bool {
    matches!(handler, Resolved::Library(name) if GENERAL_EXCEPTIONS.contains(&name.as_str()))
}
