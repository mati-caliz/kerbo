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

// Un archivo de Go del proyecto: su ruta y lo que hacen sus funciones.
pub struct GoFile<'summary> {
    pub path: &'summary str,
    pub taints: &'summary [TaintMethod],
}

// El paquete de un archivo es su carpeta: `internal/store/db.go` es `internal/store`.
pub fn package_of(path: &str) -> String {
    let unified = path.replace(WINDOWS_SEPARATOR, &PATH_SEPARATOR.to_string());
    let mut segments: Vec<&str> = unified
        .split(PATH_SEPARATOR)
        .filter(|segment| !segment.is_empty() && *segment != CURRENT)
        .collect();
    segments.pop();
    segments.join("/")
}

// Cada final de la ruta de una carpeta apunta a ella, salvo que lo compartan dos: así
// `github.com/yo/app/internal/store` encuentra `internal/store` sin saber el nombre del módulo.
fn suffixes(packages: &[String]) -> HashMap<String, Option<String>> {
    let mut found: HashMap<String, Option<String>> = HashMap::new();
    for package in packages {
        let segments: Vec<&str> = package.split(PATH_SEPARATOR).collect();
        for start in 0..segments.len() {
            let suffix = segments.get(start..).unwrap_or_default().join("/");
            record_suffix(&mut found, suffix, package);
        }
    }
    found
}

fn record_suffix(found: &mut HashMap<String, Option<String>>, suffix: String, package: &str) {
    let entry = found.entry(suffix).or_insert_with(|| Some(package.to_owned()));
    if entry.as_deref() != Some(package) {
        *entry = None;
    }
}

fn resolve_import<'map>(path: &str, suffixes: &'map HashMap<String, Option<String>>) -> Option<&'map str> {
    let segments: Vec<&str> = path.split(PATH_SEPARATOR).collect();
    (0..segments.len()).find_map(|start| {
        let suffix = segments.get(start..).unwrap_or_default().join("/");
        suffixes.get(&suffix).map(|package| package.as_deref())
    })?
}

// `@f` es del mismo paquete; `ruta/del/paquete#f`, de la carpeta que termina como esa ruta.
fn absolute(
    target: &str,
    caller_package: &str,
    suffixes: &HashMap<String, Option<String>>,
) -> Option<String> {
    if let Some(local) = target.strip_prefix(LOCAL_PREFIX) {
        return Some(format!("{caller_package}{QUALIFIER_SEPARATOR}{local}"));
    }
    let (path, member) = target.split_once(MEMBER_SEPARATOR)?;
    let package = resolve_import(path, suffixes)?;
    Some(format!("{package}{QUALIFIER_SEPARATOR}{member}"))
}

struct Resolution<'context> {
    declared: &'context HashMap<String, MethodId>,
    suffixes: &'context HashMap<String, Option<String>>,
}

fn callees(call: &TaintCall, caller_package: &str, resolution: &Resolution<'_>) -> Callees {
    let Receiver::Type(target) = &call.receiver else {
        return Callees::Library;
    };
    absolute(target, caller_package, resolution.suffixes)
        .and_then(|name| resolution.declared.get(&name).copied())
        .map_or(Callees::Library, |id| Callees::Project(vec![id]))
}

// Las consultas, comandos, rutas y respuestas a los que llega algo que mandó el usuario, por archivo.
pub fn go_tainted(files: &[GoFile<'_>]) -> Vec<Vec<LintFinding>> {
    if !mentions_source(files.iter().flat_map(|file| file.taints.iter())) {
        return files.iter().map(|_| Vec::new()).collect();
    }
    let packages: Vec<String> = files.iter().map(|file| package_of(file.path)).collect();
    let methods: Vec<(MethodId, &TaintMethod)> = files
        .iter()
        .enumerate()
        .flat_map(|(file, go)| {
            go.taints
                .iter()
                .enumerate()
                .map(move |(method, taint)| ((file, method), taint))
        })
        .collect();
    let declared: HashMap<String, MethodId> = methods
        .iter()
        .filter_map(|(id, method)| {
            let package = packages.get(id.0)?;
            Some((format!("{package}{QUALIFIER_SEPARATOR}{}", method.name), *id))
        })
        .collect();
    let suffixes = suffixes(&packages);
    let resolution = Resolution {
        declared: &declared,
        suffixes: &suffixes,
    };
    let candidates = methods
        .iter()
        .map(|(id, method)| {
            let caller_package = packages.get(id.0).map(String::as_str).unwrap_or_default();
            let per_call = method
                .calls
                .iter()
                .map(|call| callees(call, caller_package, &resolution))
                .collect();
            (*id, per_call)
        })
        .collect();
    Engine::new(methods, candidates).findings(files.len(), |sink| match sink.receiver {
        SinkReceiver::Known(kind) => Some(kind),
        SinkReceiver::Type(_) | SinkReceiver::StaticField { .. } | SinkReceiver::Constructor(_) => None,
    })
}
