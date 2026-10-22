use std::collections::{HashMap, HashSet};

use super::taint_reach::{Reach, Tainted, evaluate, substitute};
use crate::lint::taint::{Atom, Flow, SinkKind, TaintCall, TaintMethod, TaintSink};
use crate::lint::{LintFinding, LintKind};
use crate::rules::TaintRule;

// Las vueltas para que lo que devuelve un método llegue a quien lo llama a través de otros; alcanza
// de sobra para las cadenas de un proyecto real y corta la recursión.
const MAX_PASSES: usize = 16;

// Un método: el archivo y su posición en el resumen de ese archivo.
pub type MethodId = (usize, usize);

// Adónde va una llamada: a métodos del proyecto con cuerpo, a uno que implementa un framework (un
// repositorio de Spring Data: lo que devuelve sale de la base, no de los argumentos) o a una
// librería, que devuelve lo que traen el receptor y los argumentos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Callees {
    Project(Vec<MethodId>),
    Framework,
    Library,
}

// Si algo del usuario entra en alguna parte del proyecto: sin eso, el cruce no hace falta.
pub fn mentions_source<'summary>(mut methods: impl Iterator<Item = &'summary TaintMethod>) -> bool {
    let has = |flow: &Flow| flow.atoms.iter().any(|(atom, _)| *atom == Atom::Source);
    methods.any(|method| {
        has(&method.returns)
            || method
                .calls
                .iter()
                .any(|call| has(&call.receiver_flow) || call.arguments.iter().any(has))
            || method.sinks.iter().any(|sink| sink.arguments.iter().any(has))
    })
}

// El cruce de los resúmenes de todos los métodos, igual para todos los lenguajes: cada uno da a
// dónde va cada llamada y de qué tipo es cada sumidero.
pub struct Engine<'summary> {
    methods: Vec<(MethodId, &'summary TaintMethod)>,
    candidates: HashMap<MethodId, Vec<Callees>>,
    returns: HashMap<MethodId, Reach>,
    tainted: HashMap<MethodId, Tainted>,
}

impl<'summary> Engine<'summary> {
    pub fn new(
        methods: Vec<(MethodId, &'summary TaintMethod)>,
        candidates: HashMap<MethodId, Vec<Callees>>,
    ) -> Self {
        Self {
            methods,
            candidates,
            returns: HashMap::new(),
            tainted: HashMap::new(),
        }
    }

    // Lo que llega del usuario a cada sumidero, por archivo.
    pub fn findings(
        mut self,
        files: usize,
        kind_of: impl Fn(&TaintSink) -> Option<SinkKind>,
    ) -> Vec<Vec<LintFinding>> {
        self.settle_returns();
        self.settle_parameters();
        let mut found: Vec<Vec<LintFinding>> = (0..files).map(|_| Vec::new()).collect();
        for (id, method) in &self.methods {
            let values = self.call_values(*id, method);
            let tainted = self.tainted.get(id).copied().unwrap_or_default();
            let reported = method
                .sinks
                .iter()
                .filter_map(|sink| kind_of(sink).and_then(|kind| finding(sink, kind, &values, tainted)));
            if let Some(in_file) = found.get_mut(id.0) {
                in_file.extend(reported);
            }
        }
        // Un sumidero dentro de un bucle se mira en cada vuelta: avisa una sola vez.
        for in_file in &mut found {
            let mut seen = HashSet::new();
            in_file.retain(|finding| seen.insert((finding.line, finding.column, finding.kind.rule())));
        }
        found
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

    // Los parámetros de otros métodos a los que este método les pasa algo del usuario, y para qué
    // sumideros sigue siendo peligroso.
    fn reached_parameters(&self, id: MethodId, method: &TaintMethod) -> Vec<(MethodId, usize, u8)> {
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
                .map(|(position, carried)| (position, carried.dangers(tainted)))
                .filter(|(_, dangers)| *dangers != 0);
            for (position, dangers) in contaminated {
                reached.extend(found.iter().map(|callee| (*callee, position, dangers)));
            }
        }
        reached
    }

    fn settle_parameters(&mut self) {
        for _ in 0..MAX_PASSES {
            let reached: Vec<(MethodId, usize, u8)> = self
                .methods
                .iter()
                .flat_map(|(id, method)| self.reached_parameters(*id, method))
                .collect();
            let mut changed = false;
            for (callee, position, dangers) in reached {
                changed |= self.tainted.entry(callee).or_default().mark(position, dangers);
            }
            if !changed {
                break;
            }
        }
    }
}

// Si algo del usuario que sigue siendo peligroso para ese tipo llega a lo que el sumidero usa: en
// una consulta, sólo el texto (los parámetros con `?` son seguros).
fn finding(sink: &TaintSink, kind: SinkKind, values: &[Reach], tainted: Tainted) -> Option<LintFinding> {
    let checked = match kind {
        SinkKind::Sql => sink.arguments.get(..1).unwrap_or_default(),
        SinkKind::Command | SinkKind::Path | SinkKind::Html => sink.arguments.as_slice(),
    };
    checked
        .iter()
        .any(|argument| evaluate(argument, values).reaches(kind, tainted))
        .then(|| {
            let rule = match kind {
                SinkKind::Sql => TaintRule::SqlInjection,
                SinkKind::Command => TaintRule::CommandInjection,
                SinkKind::Path => TaintRule::PathTraversal,
                SinkKind::Html => TaintRule::CrossSiteScripting,
            };
            LintFinding {
                kind: LintKind::Taint(rule),
                line: sink.line,
                column: sink.column,
                subject: sink.callee.clone(),
                fix: None,
            }
        })
}
