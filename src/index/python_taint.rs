use std::collections::HashMap;
use std::collections::hash_map::Entry;

use super::taint_engine::{Callees, Engine, MethodId, mentions_source};
use super::templates::Templates;
use crate::lint::LintFinding;
use crate::lint::java_receivers::Receiver;
use crate::lint::taint::responses::known_or_template;
use crate::lint::taint::{TaintCall, TaintMethod};

const LOCAL_PREFIX: char = '@';
const QUALIFIER_SEPARATOR: char = '.';
const PATH_SEPARATORS: &[char] = &['/', '\\'];
const PYTHON_EXTENSION: &str = ".py";
const PACKAGE_INIT: &str = "__init__";

// Un archivo de Python del proyecto: su ruta relativa y lo que hacen sus funciones con los datos.
pub struct PythonModule<'summary> {
    pub path: &'summary str,
    pub taints: &'summary [TaintMethod],
}

// `pkg/mod.py` es `pkg.mod`, y `pkg/__init__.py` es `pkg`.
pub fn module_name(path: &str) -> String {
    let without_extension = path.strip_suffix(PYTHON_EXTENSION).unwrap_or(path);
    let mut segments: Vec<&str> = without_extension
        .split(PATH_SEPARATORS)
        .filter(|segment| !segment.is_empty() && *segment != ".")
        .collect();
    if segments.last() == Some(&PACKAGE_INIT) {
        segments.pop();
    }
    segments.join(".")
}

// Las funciones del proyecto por nombre completo, y también por cada sufijo de al menos dos partes
// (con un `src/` adelante, `src.pkg.mod.f` también se llama `pkg.mod.f`). Un sufijo que nombra a más
// de una no se usa.
struct Declared {
    by_name: HashMap<String, Option<MethodId>>,
}

impl Declared {
    fn new(modules: &[String], methods: &[(MethodId, &TaintMethod)]) -> Self {
        let mut by_name: HashMap<String, Option<MethodId>> = HashMap::new();
        for (id, method) in methods {
            let Some(module) = modules.get(id.0) else {
                continue;
            };
            let full = format!("{module}{QUALIFIER_SEPARATOR}{}", method.name);
            let segments: Vec<&str> = full.split(QUALIFIER_SEPARATOR).collect();
            for start in 0..segments.len().saturating_sub(1) {
                let suffix = segments.get(start..).unwrap_or_default().join(".");
                register(&mut by_name, suffix, *id, start == 0);
            }
        }
        Self { by_name }
    }

    fn find(&self, name: &str) -> Option<MethodId> {
        self.by_name.get(name).copied().flatten()
    }
}

// Un nombre completo manda; un sufijo que ya nombraba a otra función queda sin dueño.
fn register(by_name: &mut HashMap<String, Option<MethodId>>, name: String, id: MethodId, exact: bool) {
    match by_name.entry(name) {
        Entry::Vacant(slot) => {
            slot.insert(Some(id));
        }
        Entry::Occupied(mut slot) => {
            slot.insert(exact.then_some(id));
        }
    }
}

// A qué nombre completo apunta una llamada: lo del propio archivo (`@f`) y lo relativo (`.mod.f`)
// se completan con el módulo de quien llama.
fn absolute(target: &str, caller_module: &str) -> String {
    if let Some(local) = target.strip_prefix(LOCAL_PREFIX) {
        return format!("{caller_module}{QUALIFIER_SEPARATOR}{local}");
    }
    let dots = target
        .chars()
        .take_while(|letter| *letter == QUALIFIER_SEPARATOR)
        .count();
    if dots == 0 {
        return target.to_owned();
    }
    let mut package: Vec<&str> = caller_module.split(QUALIFIER_SEPARATOR).collect();
    for _ in 0..dots {
        package.pop();
    }
    let rest = target.get(dots..).unwrap_or_default();
    package.push(rest);
    package.join(".")
}

fn callees(call: &TaintCall, caller_module: &str, declared: &Declared) -> Callees {
    let Receiver::Type(target) = &call.receiver else {
        return Callees::Library;
    };
    declared
        .find(&absolute(target, caller_module))
        .map_or(Callees::Library, |id| Callees::Project(vec![id]))
}

// Las consultas, comandos, rutas y respuestas a los que llega algo que mandó el usuario, por archivo.
pub fn python_tainted(files: &[PythonModule<'_>], templates: &Templates) -> Vec<Vec<LintFinding>> {
    if !mentions_source(files.iter().flat_map(|file| file.taints.iter())) {
        return files.iter().map(|_| Vec::new()).collect();
    }
    let modules: Vec<String> = files.iter().map(|file| module_name(file.path)).collect();
    let methods: Vec<(MethodId, &TaintMethod)> = files
        .iter()
        .enumerate()
        .flat_map(|(file, module)| {
            module
                .taints
                .iter()
                .enumerate()
                .map(move |(method, taint)| ((file, method), taint))
        })
        .collect();
    let declared = Declared::new(&modules, &methods);
    let candidates = methods
        .iter()
        .map(|(id, method)| {
            let caller_module = modules.get(id.0).map(String::as_str).unwrap_or_default();
            let per_call = method
                .calls
                .iter()
                .map(|call| callees(call, caller_module, &declared))
                .collect();
            (*id, per_call)
        })
        .collect();
    Engine::new(methods, candidates).findings(files.len(), |sink| {
        known_or_template(sink, |template| templates.is_raw(template))
    })
}
