use std::collections::HashMap;

use super::taint_engine::{Callees, Engine, MethodId, mentions_source};
use crate::lint::LintFinding;
use crate::lint::java_receivers::Receiver;
use crate::lint::taint::{SinkReceiver, TaintCall, TaintMethod};

const LOCAL_PREFIX: char = '@';
const MEMBER_SEPARATOR: char = '#';
const QUALIFIER_SEPARATOR: char = '.';
const PATH_SEPARATOR: char = '/';
const WINDOWS_SEPARATOR: char = '\\';
const CURRENT: &str = ".";
const PARENT: &str = "..";
const INDEX: &str = "index";
const EXTENSIONS: &[&str] = &[".tsx", ".ts", ".mts", ".cts", ".jsx", ".js", ".mjs", ".cjs"];

// Un archivo de JavaScript o TypeScript del proyecto: su ruta y lo que hacen sus funciones.
pub struct ScriptModule<'summary> {
    pub path: &'summary str,
    pub taints: &'summary [TaintMethod],
}

// `src/utils.ts` es `src/utils`, y `src/lib/index.js` es `src/lib`: como los resuelve Node.
pub fn module_name(path: &str) -> String {
    let unified = path.replace(WINDOWS_SEPARATOR, &PATH_SEPARATOR.to_string());
    let without_extension = EXTENSIONS
        .iter()
        .find_map(|extension| unified.strip_suffix(extension))
        .unwrap_or(&unified);
    let segments: Vec<&str> = without_extension
        .split(PATH_SEPARATOR)
        .filter(|segment| !segment.is_empty() && *segment != CURRENT)
        .collect();
    let trimmed = match segments.split_last() {
        Some((last, rest)) if *last == INDEX => rest,
        _ => segments.as_slice(),
    };
    trimmed.join("/")
}

// La carpeta de un módulo, sin el último segmento.
fn directory_of(module: &str) -> Vec<&str> {
    let mut segments: Vec<&str> = module
        .split(PATH_SEPARATOR)
        .filter(|segment| !segment.is_empty())
        .collect();
    segments.pop();
    segments
}

// A qué función del proyecto apunta una llamada: lo del propio archivo (`@f`) y lo importado con
// una ruta relativa (`./utils#escape`) se completan con el módulo de quien llama.
fn absolute(target: &str, caller_module: &str) -> Option<String> {
    if let Some(local) = target.strip_prefix(LOCAL_PREFIX) {
        return Some(format!("{caller_module}{QUALIFIER_SEPARATOR}{local}"));
    }
    let (specifier, member) = target.split_once(MEMBER_SEPARATOR)?;
    let mut resolved = directory_of(caller_module);
    for segment in specifier.split(PATH_SEPARATOR) {
        match segment {
            CURRENT | "" => {}
            PARENT => {
                resolved.pop();
            }
            other => resolved.push(other),
        }
    }
    let module = module_name(&resolved.join("/"));
    Some(format!("{module}{QUALIFIER_SEPARATOR}{member}"))
}

fn callees(call: &TaintCall, caller_module: &str, declared: &HashMap<String, MethodId>) -> Callees {
    let Receiver::Type(target) = &call.receiver else {
        return Callees::Library;
    };
    absolute(target, caller_module)
        .and_then(|name| declared.get(&name).copied())
        .map_or(Callees::Library, |id| Callees::Project(vec![id]))
}

// Las consultas, comandos, rutas y respuestas a los que llega algo que mandó el usuario, por archivo.
pub fn ecmascript_tainted(files: &[ScriptModule<'_>]) -> Vec<Vec<LintFinding>> {
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
    let declared: HashMap<String, MethodId> = methods
        .iter()
        .filter_map(|(id, method)| {
            let module = modules.get(id.0)?;
            Some((format!("{module}{QUALIFIER_SEPARATOR}{}", method.name), *id))
        })
        .collect();
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
    Engine::new(methods, candidates).findings(files.len(), |sink| match sink.receiver {
        SinkReceiver::Known(kind) => Some(kind),
        SinkReceiver::Type(_) | SinkReceiver::StaticField { .. } | SinkReceiver::Constructor(_) => None,
    })
}
