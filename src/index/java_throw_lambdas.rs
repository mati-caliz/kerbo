use std::collections::{HashMap, HashSet};

use super::java::Declaration;
use super::java_project::IndexedSummary;
use crate::lint::java_calls::Body;
use crate::lint::java_lambda_targets::LambdaSite;
use crate::lint::java_owner::OwnerKind;
use crate::lint::java_receivers::simple_type_name;

// Las interfaces de un solo método abstracto a las que alguna lambda o referencia a método del
// proyecto puede apuntar: una con esa cantidad de parámetros en un archivo que nombra la interfaz
// (o un subtipo suyo), o pasada a un método que recibe la interfaz. Sin compilar no se sabe más, y
// alcanza con que pueda para no opinar.
pub(super) fn lambda_targets<'summary>(files: &[IndexedSummary<'summary>]) -> HashSet<&'summary str> {
    let abstract_methods = single_abstract_methods(files);
    let children = children_of(files);
    abstract_methods
        .into_iter()
        .filter(|(interface, abstract_method)| {
            let aliases = with_subtypes(interface, &children);
            let receivers: HashSet<&str> = files
                .iter()
                .flat_map(|file| &file.summary.lambda_targets.parameter_types)
                .filter(|(_, written)| aliases.contains(written.as_str()))
                .map(|(method, _)| method.as_str())
                .collect();
            files.iter().any(|file| {
                let targets = &file.summary.lambda_targets;
                let names_interface = targets
                    .named_types
                    .iter()
                    .any(|named| aliases.contains(named.as_str()));
                targets.sites.iter().any(|site| {
                    fits(site, abstract_method)
                        && (names_interface
                            || site
                                .argument_of
                                .as_deref()
                                .is_some_and(|call| receivers.contains(call)))
                })
            })
        })
        .map(|(interface, _)| interface)
        .collect()
}

fn fits(site: &LambdaSite, method: &Declaration) -> bool {
    site.arity.is_none_or(|arity| arity == method.parameters.len())
}

fn single_abstract_methods<'summary>(
    files: &[IndexedSummary<'summary>],
) -> HashMap<&'summary str, &'summary Declaration> {
    let mut by_interface: HashMap<&str, Vec<&Declaration>> = HashMap::new();
    for declaration in files.iter().flat_map(|file| &file.summary.declarations) {
        if declaration.body == Body::Abstract && declaration.owner.kind == OwnerKind::Interface {
            by_interface
                .entry(declaration.owner.name.as_str())
                .or_default()
                .push(declaration);
        }
    }
    by_interface
        .into_iter()
        .filter_map(|(interface, methods)| match methods.as_slice() {
            [single] => Some((interface, *single)),
            _ => None,
        })
        .collect()
}

fn children_of<'summary>(files: &[IndexedSummary<'summary>]) -> HashMap<String, Vec<&'summary str>> {
    let mut children: HashMap<String, Vec<&str>> = HashMap::new();
    for (name, parents) in files.iter().flat_map(|file| &file.summary.hierarchy) {
        for parent in parents {
            children
                .entry(simple_type_name(parent))
                .or_default()
                .push(name.as_str());
        }
    }
    children
}

fn with_subtypes<'name>(
    interface: &'name str,
    children: &HashMap<String, Vec<&'name str>>,
) -> HashSet<&'name str> {
    let mut found = HashSet::from([interface]);
    let mut pending = vec![interface];
    while let Some(current) = pending.pop() {
        for child in children.get(current).into_iter().flatten() {
            if found.insert(child) {
                pending.push(child);
            }
        }
    }
    found
}
