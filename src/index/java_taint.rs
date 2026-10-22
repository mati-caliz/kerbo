use std::collections::HashMap;

use super::java::{Position, lint_at};
use super::java_project::{IndexedSummary, Project};
use super::java_taint_reach::{Reach, Tainted, evaluate, substitute};
use crate::lint::LintFinding;
use crate::lint::java_calls::Body;
use crate::lint::java_owner::OwnerKind;
use crate::lint::java_receivers::Receiver;
use crate::lint::java_taint::tables::{constructor_sink, method_sink};
use crate::lint::java_taint::{Atom, Flow, SinkKind, SinkReceiver, TaintCall, TaintMethod, TaintSink};
use crate::rules::JavaRule;

// Las vueltas para que lo que devuelve un método llegue a quien lo llama a través de otros; alcanza
// de sobra para las cadenas de un proyecto real y corta la recursión.
const MAX_PASSES: usize = 16;

type MethodId = (usize, usize);
type AbstractMethods<'summary> = HashMap<(&'summary str, usize), Vec<&'summary str>>;

// Adónde va una llamada: a métodos del proyecto con cuerpo, a uno abstracto que implementa un
// framework (un repositorio de Spring Data: lo que devuelve sale de la base, no de los
// argumentos) o a una librería, que devuelve lo que traen el receptor y los argumentos.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Callees {
    Project(Vec<MethodId>),
    Framework,
    Library,
}

fn mentions_source(method: &TaintMethod) -> bool {
    let has = |flow: &Flow| flow.atoms.contains(&Atom::Source) || flow.escaped.contains(&Atom::Source);
    has(&method.returns)
        || method
            .calls
            .iter()
            .any(|call| has(&call.receiver_flow) || call.arguments.iter().any(has))
        || method.sinks.iter().any(|sink| sink.arguments.iter().any(has))
}

struct Taints<'summary> {
    methods: Vec<(MethodId, &'summary TaintMethod)>,
    candidates: HashMap<MethodId, Vec<Callees>>,
    fields: HashMap<(&'summary str, &'summary str), &'summary str>,
    returns: HashMap<MethodId, Reach>,
    tainted: HashMap<MethodId, Tainted>,
}

// Las consultas, comandos y rutas a los que llega algo que mandó el usuario, por archivo.
pub fn tainted_sinks(files: &[IndexedSummary<'_>], project: &Project<'_>) -> Vec<Vec<LintFinding>> {
    let mut found: Vec<Vec<LintFinding>> = files.iter().map(|_| Vec::new()).collect();
    let any_source = files
        .iter()
        .flat_map(|indexed| indexed.summary.taints.iter())
        .any(mentions_source);
    if !any_source {
        return found;
    }
    let mut taints = Taints::new(files, project);
    taints.settle_returns();
    taints.settle_parameters();
    for (id, method) in &taints.methods {
        let values = taints.call_values(*id, method);
        for sink in &method.sinks {
            if let Some(finding) = taints.finding(*id, sink, &values)
                && let Some(in_file) = found.get_mut(id.0)
            {
                in_file.push(finding);
            }
        }
    }
    found
}

// Los métodos del proyecto por nombre y aridad: los que tienen cuerpo, con su clase, y los
// abstractos de una interfaz, que implementa un framework si nadie los implementa en el código.
struct Declared<'summary> {
    bodies: HashMap<(&'summary str, usize), Vec<(MethodId, &'summary str)>>,
    abstracts: AbstractMethods<'summary>,
}

impl<'summary> Declared<'summary> {
    fn new(files: &[IndexedSummary<'summary>], methods: &[(MethodId, &'summary TaintMethod)]) -> Self {
        let mut bodies: HashMap<(&str, usize), Vec<(MethodId, &str)>> = HashMap::new();
        for (id, method) in methods {
            bodies
                .entry((method.name.as_str(), method.arity))
                .or_default()
                .push((*id, method.owner.as_str()));
        }
        let mut abstracts: AbstractMethods<'_> = HashMap::new();
        for declaration in files
            .iter()
            .flat_map(|indexed| indexed.summary.declarations.iter())
            .filter(|declaration| {
                declaration.body == Body::Abstract && declaration.owner.kind == OwnerKind::Interface
            })
        {
            abstracts
                .entry((declaration.name.as_str(), declaration.parameters.len()))
                .or_default()
                .push(declaration.owner.name.as_str());
        }
        Self { bodies, abstracts }
    }
}

impl<'summary> Taints<'summary> {
    fn new(files: &[IndexedSummary<'summary>], project: &Project<'_>) -> Self {
        let methods: Vec<(MethodId, &TaintMethod)> = files
            .iter()
            .enumerate()
            .flat_map(|(file, indexed)| {
                indexed
                    .summary
                    .taints
                    .iter()
                    .enumerate()
                    .map(move |(method, taint)| ((file, method), taint))
            })
            .collect();
        let declared = Declared::new(files, &methods);
        let candidates = methods
            .iter()
            .map(|(id, method)| {
                let per_call = method
                    .calls
                    .iter()
                    .map(|call| callees(call, id.0, &declared, project))
                    .collect();
                (*id, per_call)
            })
            .collect();
        let fields = files
            .iter()
            .flat_map(|indexed| indexed.summary.fields.iter())
            .map(|field| {
                (
                    (field.owner.name.as_str(), field.name.as_str()),
                    field.type_name.as_str(),
                )
            })
            .collect();
        Self {
            methods,
            candidates,
            fields,
            returns: HashMap::new(),
            tainted: HashMap::new(),
        }
    }

    fn callees_of(&self, id: MethodId, index: usize) -> &Callees {
        self.candidates
            .get(&id)
            .and_then(|per_call| per_call.get(index))
            .unwrap_or(&Callees::Library)
    }

    // Lo que puede llevar cada llamada de un método, en orden: los argumentos de una llamada sólo
    // usan llamadas anteriores.
    fn call_values(&self, id: MethodId, method: &TaintMethod) -> Vec<Reach> {
        let mut values: Vec<Reach> = Vec::with_capacity(method.calls.len());
        for (index, call) in method.calls.iter().enumerate() {
            let result = self.call_result(call, self.callees_of(id, index), &values);
            values.push(result);
        }
        values
    }

    fn call_result(&self, call: &TaintCall, callees: &Callees, values: &[Reach]) -> Reach {
        let arguments: Vec<Reach> = call.arguments.iter().map(|flow| evaluate(flow, values)).collect();
        let mut result = Reach::default();
        match callees {
            Callees::Framework => {}
            Callees::Project(found) => {
                for returned in found.iter().filter_map(|callee| self.returns.get(callee)) {
                    result.absorb(substitute(*returned, &arguments));
                }
            }
            Callees::Library => {
                result.absorb(evaluate(&call.receiver_flow, values));
                for argument in &arguments {
                    result.absorb(*argument);
                }
            }
        }
        result
    }

    fn settle_returns(&mut self) {
        for _ in 0..MAX_PASSES {
            let updated: Vec<(MethodId, Reach)> = self
                .methods
                .iter()
                .map(|(id, method)| (*id, evaluate(&method.returns, &self.call_values(*id, method))))
                .filter(|(id, returned)| self.returns.get(id) != Some(returned))
                .collect();
            if updated.is_empty() {
                break;
            }
            self.returns.extend(updated);
        }
    }

    // Los parámetros de otros métodos a los que este método les pasa algo del usuario, y si va sin
    // escapar.
    fn reached_parameters(&self, id: MethodId, method: &TaintMethod) -> Vec<(MethodId, usize, bool)> {
        let values = self.call_values(id, method);
        let tainted = self.tainted.get(&id).copied().unwrap_or_default();
        let mut reached = Vec::new();
        for (index, call) in method.calls.iter().enumerate() {
            let Callees::Project(found) = self.callees_of(id, index) else {
                continue;
            };
            let contaminated = call
                .arguments
                .iter()
                .map(|argument| evaluate(argument, &values))
                .enumerate()
                .filter(|(_, carried)| carried.any_user(tainted));
            for (position, carried) in contaminated {
                let raw = carried.raw_user(tainted);
                reached.extend(found.iter().map(|callee| (*callee, position, raw)));
            }
        }
        reached
    }

    fn settle_parameters(&mut self) {
        for _ in 0..MAX_PASSES {
            let reached: Vec<(MethodId, usize, bool)> = self
                .methods
                .iter()
                .flat_map(|(id, method)| self.reached_parameters(*id, method))
                .collect();
            let mut changed = false;
            for (callee, position, raw) in reached {
                changed |= self.tainted.entry(callee).or_default().mark(position, raw);
            }
            if !changed {
                break;
            }
        }
    }

    fn finding(&self, id: MethodId, sink: &TaintSink, values: &[Reach]) -> Option<LintFinding> {
        let kind = match &sink.receiver {
            SinkReceiver::Type(declared) => method_sink(declared, &sink.callee),
            SinkReceiver::StaticField { owner, field } => self
                .fields
                .get(&(owner.as_str(), field.as_str()))
                .and_then(|declared| method_sink(declared, &sink.callee)),
            SinkReceiver::Constructor(created) => constructor_sink(created),
        }?;
        let checked = match kind {
            SinkKind::Sql => sink.arguments.get(..1).unwrap_or_default(),
            SinkKind::Command | SinkKind::Path | SinkKind::Html => sink.arguments.as_slice(),
        };
        let tainted = self.tainted.get(&id).copied().unwrap_or_default();
        checked
            .iter()
            .any(|argument| {
                let carried = evaluate(argument, values);
                if kind == SinkKind::Html {
                    carried.raw_user(tainted)
                } else {
                    carried.any_user(tainted)
                }
            })
            .then(|| {
                let rule = match kind {
                    SinkKind::Sql => JavaRule::SqlInjection,
                    SinkKind::Command => JavaRule::CommandInjection,
                    SinkKind::Path => JavaRule::PathTraversal,
                    SinkKind::Html => JavaRule::CrossSiteScripting,
                };
                let position = Position {
                    line: sink.line,
                    column: sink.column,
                };
                lint_at(rule, &position, sink.callee.clone())
            })
    }
}

// Los métodos del proyecto a los que puede ir una llamada: los de su tipo, los que hereda y los que
// lo redefinen (una interfaz va a sus implementaciones). Sin tipo conocido, a ninguno: se toma
// como una llamada a una librería. Un tipo del archivo de quien llama tapa a los que se llaman
// igual en otros archivos (cada servlet puede tener su clase interna `Test`).
fn callees(call: &TaintCall, caller_file: usize, declared: &Declared<'_>, project: &Project<'_>) -> Callees {
    let key = (call.name.as_str(), call.arguments.len());
    let mut reached = declared
        .bodies
        .get(&key)
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter(|(_, owner)| match &call.receiver {
            Receiver::Unknown => false,
            Receiver::Type(written) => {
                project.is_ancestor(owner, written) || project.is_ancestor(written, owner)
            }
            Receiver::Scope(enclosing) => enclosing.iter().any(|inner| project.is_ancestor(owner, inner)),
        })
        .map(|(id, _)| *id)
        .collect::<Vec<MethodId>>();
    if reached.iter().any(|(file, _)| *file == caller_file) {
        reached.retain(|(file, _)| *file == caller_file);
    }
    if !reached.is_empty() {
        return Callees::Project(reached);
    }
    let implemented_by_framework = matches!(&call.receiver, Receiver::Type(written)
        if declared
            .abstracts
            .get(&key)
            .is_some_and(|owners| owners.iter().any(|owner| project.is_ancestor(owner, written))));
    if implemented_by_framework {
        Callees::Framework
    } else {
        Callees::Library
    }
}
