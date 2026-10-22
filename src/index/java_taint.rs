use std::collections::HashMap;

use super::java_project::{IndexedSummary, Project};
use super::taint_engine::{Callees, Engine, MethodId, mentions_source};
use crate::lint::LintFinding;
use crate::lint::java_calls::Body;
use crate::lint::java_owner::OwnerKind;
use crate::lint::java_receivers::Receiver;
use crate::lint::java_taint::tables::{constructor_sink, method_sink};
use crate::lint::taint::{SinkReceiver, TaintCall, TaintMethod};

type AbstractMethods<'summary> = HashMap<(&'summary str, usize), Vec<&'summary str>>;

// Las consultas, comandos, rutas y respuestas a los que llega algo que mandó el usuario, por archivo.
pub fn tainted_sinks(files: &[IndexedSummary<'_>], project: &Project<'_>) -> Vec<Vec<LintFinding>> {
    if !mentions_source(files.iter().flat_map(|indexed| indexed.summary.taints.iter())) {
        return files.iter().map(|_| Vec::new()).collect();
    }
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
    let project_methods = Declared::new(files, &methods);
    let candidates = methods
        .iter()
        .map(|(id, method)| {
            let per_call = method
                .calls
                .iter()
                .map(|call| callees(call, id.0, &project_methods, project))
                .collect();
            (*id, per_call)
        })
        .collect();
    let fields: HashMap<(&str, &str), &str> = files
        .iter()
        .flat_map(|indexed| indexed.summary.fields.iter())
        .map(|field| {
            (
                (field.owner.name.as_str(), field.name.as_str()),
                field.type_name.as_str(),
            )
        })
        .collect();
    Engine::new(methods, candidates).findings(files.len(), |sink| match &sink.receiver {
        SinkReceiver::Type(declared) => method_sink(declared, &sink.callee),
        SinkReceiver::StaticField { owner, field } => fields
            .get(&(owner.as_str(), field.as_str()))
            .and_then(|declared| method_sink(declared, &sink.callee)),
        SinkReceiver::Constructor(created) => constructor_sink(created),
        SinkReceiver::Known(kind) => Some(*kind),
    })
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
